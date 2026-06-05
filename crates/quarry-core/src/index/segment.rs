//! The on-disk segment format.
//!
//! A segment is one immutable, self-contained index: dictionary, postings,
//! stored fields and statistics in a single file. Immutability is what makes
//! the design simple — nothing is ever updated in place, so there is no
//! locking, no partial-write window, and a reader never sees a half-applied
//! change. New documents go into a new segment, and [`super::merge()`] folds
//! segments together in the background.
//!
//! Layout:
//!
//! ```text
//! magic "QRYS"      4 bytes
//! version           4 bytes
//! body              varint-coded sections:
//!                     schema digest
//!                     doc count
//!                     field statistics
//!                     document field lengths
//!                     term dictionary + postings
//!                     stored fields
//! body checksum     4 bytes  (CRC32 of the body)
//! magic "QRYE"      4 bytes
//! ```
//!
//! The trailing magic is what distinguishes a truncated file from a corrupt
//! one: a write killed partway through has no footer at all, which is a clearer
//! diagnosis than a checksum mismatch.

use super::inverted::{FieldStats, InvertedIndex, TermKey};
use super::posting::PostingList;
use crate::doc::DocId;
use crate::error::{Error, Result};
use crate::schema::{FieldId, Schema};
use crate::store::blob::{BlobReader, BlobWriter};
use crate::store::checksum::{crc32, fnv1a};
use crate::store::docstore::DocStore;
use std::collections::HashMap;

/// Leading magic.
pub const MAGIC_START: &[u8; 4] = b"QRYS";
/// Trailing magic.
pub const MAGIC_END: &[u8; 4] = b"QRYE";
/// Format version this build writes and reads.
pub const FORMAT_VERSION: u32 = 4;
/// Smallest possible well-formed segment: both magics, version and checksum.
pub const MIN_SEGMENT_LEN: usize = 16;

/// What a segment records about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentMeta {
    /// Format version the file was written with.
    pub version: u32,
    /// Documents in the segment.
    pub doc_count: u32,
    /// Distinct field/term pairs.
    pub term_count: u32,
    /// Hash of the schema it was written against.
    pub schema_digest: u64,
}

/// One immutable index.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Self-description.
    pub meta: SegmentMeta,
    /// Dictionary and postings.
    pub index: InvertedIndex,
    /// Stored field values.
    pub store: DocStore,
}

impl Segment {
    /// Builds a segment from an in-memory index and store.
    #[must_use]
    pub fn new(index: InvertedIndex, store: DocStore, schema: &Schema) -> Self {
        Self {
            meta: SegmentMeta {
                version: FORMAT_VERSION,
                doc_count: index.doc_count(),
                term_count: index.term_count() as u32,
                schema_digest: schema_digest(schema),
            },
            index,
            store,
        }
    }

    /// Documents in the segment.
    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.meta.doc_count
    }

    /// Whether the segment holds no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meta.doc_count == 0
    }

    /// Serializes the segment.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let mut body = BlobWriter::new();

        body.u64(self.meta.schema_digest);
        body.varint(u64::from(self.meta.doc_count));

        // Field statistics.
        let fields = self.index.active_fields();
        body.varint(fields.len() as u64);
        for field in &fields {
            let stats = self.index.field_stats(*field);
            body.varint(u64::from(*field));
            body.varint(stats.doc_count);
            body.varint(stats.total_length);
        }

        // Per-document field lengths, grouped by field so document ids ascend
        // within each group and gap-code well.
        body.varint(fields.len() as u64);
        for field in &fields {
            body.varint(u64::from(*field));

            let mut lengths: Vec<(DocId, u32)> = (0..self.meta.doc_count)
                .filter_map(|doc| {
                    let len = self.index.field_length(doc, *field);
                    (len > 0).then_some((doc, len))
                })
                .collect();
            lengths.sort_unstable_by_key(|(doc, _)| *doc);

            body.varint(lengths.len() as u64);
            let mut previous = 0u32;
            for (doc, len) in lengths {
                body.varint(u64::from(doc - previous));
                body.varint(u64::from(len));
                previous = doc;
            }
        }

        // Term dictionary with front coding, then each posting list.
        //
        // Terms arrive sorted, so adjacent entries share a prefix far more
        // often than not: storing the shared length plus the suffix is most of
        // the dictionary's compression.
        let terms: Vec<(&TermKey, &PostingList)> = self.index.iter_terms().collect();
        body.varint(terms.len() as u64);

        let mut previous_text = String::new();
        for (key, list) in terms {
            let shared = shared_prefix_len(&previous_text, &key.text);
            body.varint(u64::from(key.field));
            body.varint(shared as u64);
            body.string(&key.text[shared..]);
            list.encode(&mut body);
            previous_text.clone_from(&key.text);
        }

        self.store.encode(&mut body);

        let body = body.finish();

        let mut out = Vec::with_capacity(body.len() + MIN_SEGMENT_LEN);
        out.extend_from_slice(MAGIC_START);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_le_bytes());
        out.extend_from_slice(MAGIC_END);
        out
    }

    /// Parses a segment, verifying its framing and checksum.
    pub fn read(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < MIN_SEGMENT_LEN {
            return Err(Error::corrupt(format!(
                "segment is {} bytes, shorter than the {MIN_SEGMENT_LEN} byte minimum",
                bytes.len()
            )));
        }

        if &bytes[..4] != MAGIC_START {
            return Err(Error::corrupt("missing leading magic; not a segment file"));
        }

        // Checked before the version, so a half-written file reports what it
        // actually is rather than a nonsense version number read from the gap.
        if &bytes[bytes.len() - 4..] != MAGIC_END {
            return Err(Error::corrupt(
                "missing trailing magic; the segment was truncated mid-write",
            ));
        }

        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != FORMAT_VERSION {
            return Err(Error::Version {
                found: version,
                expected: FORMAT_VERSION,
            });
        }

        let body = &bytes[8..bytes.len() - 8];
        let stored_checksum = u32::from_le_bytes([
            bytes[bytes.len() - 8],
            bytes[bytes.len() - 7],
            bytes[bytes.len() - 6],
            bytes[bytes.len() - 5],
        ]);
        BlobReader::verify_checksum(body, stored_checksum)?;

        let mut reader = BlobReader::new(body);

        let schema_digest = reader.u64()?;
        let doc_count = reader.varint_u32()?;

        let mut index = InvertedIndex::new();
        for _ in 0..doc_count {
            index.allocate_doc();
        }

        // Field statistics are read into a map first and applied after the
        // lengths, because `record_field_length` accumulates them and would
        // otherwise double-count.
        let stat_count = reader.varint()? as usize;
        let mut stats = HashMap::with_capacity(stat_count);
        for _ in 0..stat_count {
            let field = read_field_id(&mut reader)?;
            let doc_count = reader.varint()?;
            let total_length = reader.varint()?;
            stats.insert(
                field,
                FieldStats {
                    doc_count,
                    total_length,
                },
            );
        }

        let length_groups = reader.varint()? as usize;
        for _ in 0..length_groups {
            let field = read_field_id(&mut reader)?;
            let entries = reader.varint()? as usize;
            let mut doc = 0u32;

            for _ in 0..entries {
                let gap = reader.varint_u32()?;
                doc = doc
                    .checked_add(gap)
                    .ok_or_else(|| Error::corrupt("length entry doc id overflows u32"))?;
                let length = reader.varint_u32()?;
                index.record_field_length(doc, field, length);
            }
        }

        let term_count = reader.varint()? as usize;
        let mut previous_text = String::new();

        for _ in 0..term_count {
            let field = read_field_id(&mut reader)?;
            let shared = reader.varint()? as usize;
            let suffix = reader.string()?;

            if shared > previous_text.len() {
                return Err(Error::corrupt(format!(
                    "front-coded prefix of {shared} exceeds the previous term"
                )));
            }
            // `previous_text` is built from valid UTF-8 suffixes, but a corrupt
            // shared length could land mid-character.
            if !previous_text.is_char_boundary(shared) {
                return Err(Error::corrupt(
                    "front-coded prefix splits a utf-8 character",
                ));
            }

            let text = format!("{}{}", &previous_text[..shared], suffix);
            let list = PostingList::decode(&mut reader)?;

            for posting in &list.postings {
                if posting.doc >= doc_count {
                    return Err(Error::corrupt(format!(
                        "posting references doc {} but the segment has {doc_count}",
                        posting.doc
                    )));
                }
            }

            index.insert_list(TermKey::new(field, text.clone()), list);
            previous_text = text;
        }

        let store = DocStore::decode(&mut reader)?;

        index.set_field_stats(stats);

        Ok(Self {
            meta: SegmentMeta {
                version,
                doc_count,
                term_count: term_count as u32,
                schema_digest,
            },
            index,
            store,
        })
    }

    /// Confirms the segment was written against `schema`.
    ///
    /// Field ids are positional, so reading a segment with a schema whose
    /// fields were reordered would silently attribute terms to the wrong field.
    /// This turns that into an error.
    pub fn check_schema(&self, schema: &Schema) -> Result<()> {
        let expected = schema_digest(schema);
        if self.meta.schema_digest == expected {
            Ok(())
        } else {
            Err(Error::schema(format!(
                "segment was written against a different schema (digest {:#018x}, expected {:#018x})",
                self.meta.schema_digest, expected
            )))
        }
    }
}

fn read_field_id(reader: &mut BlobReader<'_>) -> Result<FieldId> {
    u16::try_from(reader.varint()?).map_err(|_| Error::corrupt("field id does not fit in u16"))
}

/// Length of the shared prefix of two strings, on a character boundary.
#[must_use]
pub fn shared_prefix_len(a: &str, b: &str) -> usize {
    let limit = a.len().min(b.len());
    let (a_bytes, b_bytes) = (a.as_bytes(), b.as_bytes());

    let mut i = 0;
    while i < limit && a_bytes[i] == b_bytes[i] {
        i += 1;
    }

    // Back off to a boundary so slicing the prefix is always valid.
    while i > 0 && !a.is_char_boundary(i) {
        i -= 1;
    }

    i
}

/// A hash of the schema's shape.
///
/// Covers names, types and the options that change how a term is indexed.
/// `stored` and `boost` are deliberately excluded: changing either does not
/// invalidate existing postings, so including them would force a needless
/// reindex.
#[must_use]
pub fn schema_digest(schema: &Schema) -> u64 {
    let mut buffer = Vec::new();

    for field in schema.fields() {
        buffer.extend_from_slice(&field.id.to_le_bytes());
        buffer.extend_from_slice(field.name.as_bytes());
        buffer.push(0);
        buffer.extend_from_slice(field.ty.as_str().as_bytes());
        buffer.push(u8::from(field.options.indexed));
        buffer.push(u8::from(field.options.positions));
        buffer.push(0xFF);
    }

    fnv1a(&buffer)
}

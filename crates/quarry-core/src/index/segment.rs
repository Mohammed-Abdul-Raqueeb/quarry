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

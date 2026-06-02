//! Posting lists.

use crate::doc::DocId;
use crate::error::{Error, Result};
use crate::schema::FieldId;
use crate::store::blob::{BlobReader, BlobWriter};

/// One term's appearance in one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    /// Which document.
    pub doc: DocId,
    /// How many times the term occurs in the field.
    pub term_freq: u32,
    /// Ordinals at which it occurs, ascending.
    ///
    /// Empty when the field was indexed without positions. Phrase queries need
    /// these; everything else only needs `term_freq`, which is why positions are
    /// opt-in — they roughly double a list's size.
    pub positions: Vec<u32>,
}

impl Posting {
    /// A posting with a frequency but no positions.
    #[must_use]
    pub fn new(doc: DocId, term_freq: u32) -> Self {
        Self {
            doc,
            term_freq,
            positions: Vec::new(),
        }
    }

    /// A posting with positions, taking the frequency from their count.
    #[must_use]
    pub fn with_positions(doc: DocId, positions: Vec<u32>) -> Self {
        Self {
            term_freq: positions.len() as u32,
            doc,
            positions,
        }
    }

    /// Whether positions were recorded.
    #[must_use]
    pub fn has_positions(&self) -> bool {
        !self.positions.is_empty()
    }
}

/// Every posting for one term in one field, ordered by document id.
///
/// The ordering is load-bearing: it is what lets intersection walk two lists
/// once rather than hashing one of them, and what makes the gap encoding in
/// [`PostingList::encode`] small.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostingList {
    /// Which field these postings belong to.
    pub field: FieldId,
    /// Postings, ascending by document id.
    pub postings: Vec<Posting>,
}

impl PostingList {
    /// An empty list for a field.
    #[must_use]
    pub fn new(field: FieldId) -> Self {
        Self {
            field,
            postings: Vec::new(),
        }
    }

    /// How many documents contain the term.
    ///
    /// This is the document frequency that BM25's IDF term is computed from.
    #[must_use]
    pub fn doc_freq(&self) -> u32 {
        self.postings.len() as u32
    }

    /// Total occurrences across every document.
    #[must_use]
    pub fn total_term_freq(&self) -> u64 {
        self.postings.iter().map(|p| u64::from(p.term_freq)).sum()
    }

    /// Whether the list holds no postings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.postings.is_empty()
    }

    /// Appends a posting, which must have a higher document id than the last.
    pub fn push(&mut self, posting: Posting) -> Result<()> {
        if let Some(last) = self.postings.last() {
            if posting.doc <= last.doc {
                return Err(Error::corrupt(format!(
                    "posting for doc {} follows doc {}",
                    posting.doc, last.doc
                )));
            }
        }
        self.postings.push(posting);
        Ok(())
    }

    /// The posting for `doc`, found by binary search.
    #[must_use]
    pub fn get(&self, doc: DocId) -> Option<&Posting> {
        self.postings
            .binary_search_by_key(&doc, |p| p.doc)
            .ok()
            .map(|i| &self.postings[i])
    }

    /// Whether `doc` contains the term.
    #[must_use]
    pub fn contains(&self, doc: DocId) -> bool {
        self.postings.binary_search_by_key(&doc, |p| p.doc).is_ok()
    }

    /// Document ids, ascending.
    #[must_use]
    pub fn doc_ids(&self) -> Vec<DocId> {
        self.postings.iter().map(|p| p.doc).collect()
    }

    /// Writes the list as gap-encoded varints.
    ///
    /// Document ids become gaps from the previous id, and positions become gaps
    /// within each document. A list of a million documents whose ids differ by
    /// small amounts costs about a byte each instead of four.
    pub fn encode(&self, out: &mut BlobWriter) {
        out.varint(u64::from(self.field));
        out.varint(self.postings.len() as u64);

        let mut previous_doc = 0u32;
        for posting in &self.postings {
            out.varint(u64::from(posting.doc - previous_doc));
            out.varint(u64::from(posting.term_freq));
            out.sorted_u32(&posting.positions);
            previous_doc = posting.doc;
        }
    }

    /// Reads what [`PostingList::encode`] wrote.
    pub fn decode(input: &mut BlobReader<'_>) -> Result<Self> {
        let field = u16::try_from(input.varint()?)
            .map_err(|_| Error::corrupt("field id does not fit in u16"))?;
        let count = input.varint()? as usize;

        // Clamped so a corrupt count cannot request a huge allocation before
        // the first read fails.
        let mut postings = Vec::with_capacity(count.min(4096));
        let mut doc = 0u32;

        for _ in 0..count {
            let gap = input.varint_u32()?;
            doc = doc
                .checked_add(gap)
                .ok_or_else(|| Error::corrupt("document id overflows u32"))?;
            let term_freq = input.varint_u32()?;
            let positions = input.sorted_u32()?;

            postings.push(Posting {
                doc,
                term_freq,
                positions,
            });
        }

        Ok(Self { field, postings })
    }

    /// Documents present in both lists, with `self`'s postings.
    ///
    /// A linear merge rather than a hash join: both inputs are sorted, so this
    /// is one pass with no allocation for a lookup table.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Vec<DocId> {
        let mut out = Vec::new();
        let (mut i, mut j) = (0, 0);

        while i < self.postings.len() && j < other.postings.len() {
            let left = self.postings[i].doc;
            let right = other.postings[j].doc;

            match left.cmp(&right) {
                std::cmp::Ordering::Equal => {
                    out.push(left);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        out
    }

    /// Documents present in either list.
    #[must_use]
    pub fn union(&self, other: &Self) -> Vec<DocId> {
        let mut out = Vec::with_capacity(self.postings.len() + other.postings.len());
        let (mut i, mut j) = (0, 0);

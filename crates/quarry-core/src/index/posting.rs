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

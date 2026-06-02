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

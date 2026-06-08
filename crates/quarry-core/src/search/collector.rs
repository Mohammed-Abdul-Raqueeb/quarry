//! Collecting the best matches.

use crate::doc::DocId;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// One scored match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoredDoc {
    /// Which document.
    pub doc: DocId,
    /// Its relevance score.
    pub score: f32,
}

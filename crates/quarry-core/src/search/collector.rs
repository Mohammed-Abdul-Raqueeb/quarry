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

impl ScoredDoc {
    /// A scored match.
    #[must_use]
    pub fn new(doc: DocId, score: f32) -> Self {
        Self { doc, score }
    }
}

impl Eq for ScoredDoc {}

impl Ord for ScoredDoc {
    /// Orders by score, then by *descending* document id.
    ///
    /// Two deliberate choices. `total_cmp` rather than `partial_cmp` so a NaN
    /// score — which a pathological boost could produce — orders consistently
    /// instead of panicking or silently breaking the heap's invariant. And the
    /// id tiebreak is reversed so that, once the heap is inverted for top-k,
    /// equal scores come out lowest-id-first, which makes results stable.
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| other.doc.cmp(&self.doc))
    }
}

impl PartialOrd for ScoredDoc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

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

/// Keeps the `k` highest-scoring documents seen.
///
/// A min-heap of size `k`, so memory is bounded by `k` rather than by the
/// number of matches. Sorting every match and truncating would be simpler but
/// allocates proportional to the result set — on a query matching a million
/// documents for the top ten, that is the whole cost of the query.
#[derive(Debug)]
pub struct TopK {
    heap: BinaryHeap<std::cmp::Reverse<ScoredDoc>>,
    limit: usize,
    total_hits: u64,
}

impl TopK {
    /// A collector keeping the best `limit` documents.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            // `limit + 1` because a candidate is pushed before the worst is
            // popped, so the heap momentarily holds one extra.
            heap: BinaryHeap::with_capacity(limit.saturating_add(1).min(4096)),
            limit,
            total_hits: 0,
        }
    }

    /// Offers a document to the collector.
    pub fn collect(&mut self, doc: DocId, score: f32) {
        self.total_hits += 1;

        if self.limit == 0 {
            return;
        }

        let candidate = std::cmp::Reverse(ScoredDoc::new(doc, score));

        if self.heap.len() < self.limit {
            self.heap.push(candidate);
            return;
        }

        // `peek` on the inverted heap is the *worst* kept document, so one
        // comparison decides whether the candidate belongs at all.
        if let Some(worst) = self.heap.peek() {
            if candidate.0 > worst.0 {
                self.heap.pop();
                self.heap.push(candidate);
            }
        }
    }

    /// Matches offered, including those not kept.
    #[must_use]
    pub fn total_hits(&self) -> u64 {
        self.total_hits
    }

    /// How many documents are being kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Whether nothing has been kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// The lowest score currently kept, once the heap is full.
    ///
    /// A query executor can use this to skip documents that cannot make the
    /// cut, which is where the win from a bounded collector actually shows up.
    #[must_use]
    pub fn threshold(&self) -> Option<f32> {
        if self.heap.len() < self.limit {
            None
        } else {
            self.heap.peek().map(|worst| worst.0.score)
        }
    }

    /// The kept documents, best first.
    #[must_use]
    pub fn into_sorted(self) -> Vec<ScoredDoc> {
        let mut docs: Vec<ScoredDoc> = self.heap.into_iter().map(|r| r.0).collect();
        // Descending by score; ties by ascending id, hence the reversed `cmp`.
        docs.sort_by(|a, b| b.cmp(a));
        docs
    }
}

//! Relevance scoring.
//!
//! Scoring is behind a trait so a query can be run under a different ranking
//! function without touching the executor. In practice BM25 is the right
//! default and TF-IDF is here for comparison, but the seam is also what lets
//! the tests assert a ranking with a stub scorer whose output is predictable.

pub mod bm25;
pub mod tfidf;

use std::fmt::Debug;

/// Produces a relevance contribution for one term in one document.
///
/// Called once per matching term per document, so it is on the hot path of
/// every query. It takes primitives rather than a struct because everything it
/// needs is already in registers by the time the executor reaches it.
pub trait Scorer: Debug + Send + Sync {
    /// Scores a single term occurrence set.
    ///
    /// - `term_freq` — occurrences of the term in this document's field
    /// - `doc_freq` — documents containing the term at all
    /// - `doc_count` — documents in the index
    /// - `field_length` — length of this document's field, in terms
    /// - `average_field_length` — mean length of the field across the index
    fn score(
        &self,
        term_freq: u32,
        doc_freq: u64,
        doc_count: u64,
        field_length: u32,
        average_field_length: f64,
    ) -> f32;

    /// Short name, reported in query explanations.
    fn name(&self) -> &'static str;
}

/// Counts a match as 1.0 regardless of frequency or rarity.
///
/// For a filter rather than a ranking: when results are sorted by a field, the
/// relevance score is computed and then thrown away, so computing a real one is
/// wasted work.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConstantScorer;

impl Scorer for ConstantScorer {
    fn score(&self, term_freq: u32, _: u64, _: u64, _: u32, _: f64) -> f32 {
        if term_freq > 0 {
            1.0
        } else {
            0.0
        }
    }

    fn name(&self) -> &'static str {
        "constant"
    }
}

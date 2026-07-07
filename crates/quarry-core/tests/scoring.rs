//! Relevance scoring and result collection.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::score::bm25::{Bm25, DEFAULT_B, DEFAULT_K1};
use quarry_core::score::tfidf::TfIdf;
use quarry_core::score::{ConstantScorer, Scorer};
use quarry_core::search::collector::{CountCollector, ScoredDoc, TopK};

// ---- BM25 ---------------------------------------------------------------

#[test]
fn bm25_defaults_are_the_published_values() {
    let bm25 = Bm25::default();
    assert_eq!(bm25.k1, DEFAULT_K1);
    assert_eq!(bm25.b, DEFAULT_B);
}

#[test]
fn bm25_clamps_a_negative_k1() {
    // Outside its range the formula stops being monotonic in term frequency,
    // which produces rankings that look arbitrary rather than badly tuned.
    assert_eq!(Bm25::new(-5.0, 0.5).k1, 0.0);
}

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

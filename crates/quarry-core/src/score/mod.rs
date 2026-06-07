//! Relevance scoring.
//!
//! Scoring is behind a trait so a query can be run under a different ranking
//! function without touching the executor. In practice BM25 is the right
//! default and TF-IDF is here for comparison, but the seam is also what lets
//! the tests assert a ranking with a stub scorer whose output is predictable.

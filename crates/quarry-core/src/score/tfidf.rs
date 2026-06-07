//! Classic TF-IDF scoring.
//!
//! Kept alongside BM25 because it is the simpler model and useful to compare
//! against: the difference between the two on a real corpus is mostly BM25's
//! frequency saturation, and having both makes that visible rather than
//! theoretical.
//!
//! It is not the default. TF-IDF grows without bound in term frequency, so a
//! document that repeats a word two hundred times outranks a document that uses
//! it well — which is exactly the spam that BM25's saturation resists.

use super::Scorer;

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

/// Term frequency times inverse document frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TfIdf {
    /// Whether to take the square root of term frequency.
    ///
    /// Lucene's default. A cheap, crude stand-in for BM25's saturation curve.
    pub sublinear_tf: bool,
    /// Whether to divide by the square root of field length.
    pub normalize_length: bool,
}

impl Default for TfIdf {
    fn default() -> Self {
        Self {
            sublinear_tf: true,
            normalize_length: true,
        }
    }
}

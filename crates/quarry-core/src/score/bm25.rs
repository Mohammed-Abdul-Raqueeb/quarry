//! BM25 relevance scoring.
//!
//! BM25 ranks a document for a term on three inputs:
//!
//! - **Term frequency**, with diminishing returns. A document containing a word
//!   twenty times is more relevant than one containing it twice, but nowhere
//!   near ten times more. `k1` controls how fast the curve flattens.
//! - **Inverse document frequency.** A word in every document tells you
//!   nothing; a rare word is almost the whole signal.
//! - **Field length.** A match in a six-word title means more than a match in
//!   a six-thousand-word body. `b` controls how much length is penalised.
//!
//! Reference: Robertson & Zaragoza, "The Probabilistic Relevance Framework:
//! BM25 and Beyond", 2009.

use super::Scorer;

/// Term-frequency saturation. Higher means frequency keeps mattering for longer.
pub const DEFAULT_K1: f64 = 1.2;
/// Length normalisation strength. 0 ignores length, 1 fully normalises.
pub const DEFAULT_B: f64 = 0.75;

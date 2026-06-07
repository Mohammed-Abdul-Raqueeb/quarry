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

/// The BM25 ranking function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bm25 {
    /// Saturation parameter.
    pub k1: f64,
    /// Length normalisation parameter.
    pub b: f64,
}

impl Default for Bm25 {
    fn default() -> Self {
        Self {
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }
}

impl Bm25 {
    /// BM25 with explicit parameters.
    ///
    /// `k1` is clamped non-negative and `b` to `0..=1`; outside those ranges the
    /// formula stops being monotonic in term frequency, which produces rankings
    /// that look arbitrary rather than merely badly tuned.
    #[must_use]
    pub fn new(k1: f64, b: f64) -> Self {
        Self {
            k1: k1.max(0.0),
            b: b.clamp(0.0, 1.0),
        }
    }

    /// Ignores field length entirely.
    ///
    /// Right for fields of uniform size, where normalising adds noise instead
    /// of removing it.
    #[must_use]
    pub fn without_length_normalization() -> Self {
        Self {
            k1: DEFAULT_K1,
            b: 0.0,
        }
    }

    /// The IDF weight of a term.
    ///
    /// Uses the probabilistic form with the `+0.5` smoothing, then `+1.0`
    /// inside the logarithm. Without that last term a word appearing in more
    /// than half the documents gets a *negative* weight, so adding it to a
    /// query would push matching documents down the ranking.
    #[must_use]
    pub fn idf(doc_count: u64, doc_freq: u64) -> f64 {
        if doc_count == 0 {
            return 0.0;
        }

        let n = doc_count as f64;
        // A document frequency above the corpus size means corrupt statistics;
        // clamping keeps the logarithm's argument positive.
        let df = (doc_freq as f64).min(n);

        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }
}

impl Scorer for Bm25 {
    fn score(
        &self,
        term_freq: u32,
        doc_freq: u64,
        doc_count: u64,
        field_length: u32,
        average_field_length: f64,
    ) -> f32 {
        if term_freq == 0 || doc_count == 0 {
            return 0.0;
        }

        let tf = f64::from(term_freq);
        let idf = Self::idf(doc_count, doc_freq);

        // Guard against a zero average, which a corrupt or empty statistics
        // block could produce.
        let average = if average_field_length > 0.0 {
            average_field_length
        } else {
            1.0
        };
        let length_ratio = f64::from(field_length) / average;

        let numerator = tf * (self.k1 + 1.0);
        let denominator = tf + self.k1 * (1.0 - self.b + self.b * length_ratio);

        (idf * numerator / denominator) as f32
    }

    fn name(&self) -> &'static str {
        "bm25"
    }
}

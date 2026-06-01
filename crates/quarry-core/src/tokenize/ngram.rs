//! Character n-grams.
//!
//! Two jobs that the word tokenizer cannot do:
//!
//! - **Languages without word separators.** Chinese and Japanese do not put
//!   spaces between words, so splitting on non-alphanumerics yields one
//!   enormous token. Bigrams are the standard cheap answer.
//! - **Substring and typo matching.** Indexing edge n-grams makes prefix
//!   matching a plain term lookup, which is how an as-you-type box stays fast
//!   without a wildcard scan over the dictionary.
//!
//! The cost is index size: every word becomes several terms, so postings grow
//! roughly with the n-gram range. Use it on one field, not everywhere.

use super::{Token, Tokenizer};

/// Emits every character window between `min` and `max` wide.
#[derive(Debug, Clone, Copy)]
pub struct NgramTokenizer {
    /// Shortest window.
    pub min: usize,
    /// Longest window.
    pub max: usize,
}

impl Default for NgramTokenizer {
    fn default() -> Self {
        // Bigrams and trigrams: enough to be discriminating without exploding
        // the dictionary.
        Self { min: 2, max: 3 }
    }
}

impl NgramTokenizer {
    /// An n-gram tokenizer over a window range.
    ///
    /// `min` is clamped to at least 1, and `max` to at least `min`, so a
    /// reversed or zero range degrades to something sensible rather than
    /// producing no tokens at all.
    #[must_use]
    pub fn new(min: usize, max: usize) -> Self {
        let min = min.max(1);
        Self {
            min,
            max: max.max(min),
        }
    }

    /// Fixed-width windows.
    #[must_use]
    pub fn fixed(n: usize) -> Self {
        Self::new(n, n)
    }
}

//! Stopword removal.

use super::{Token, TokenFilter};
use std::collections::HashSet;

/// The standard English stopword list, as used by Lucene and Snowball.
///
/// Every one of these appears in a large fraction of documents, so it
/// contributes almost nothing to a score while taking up a posting list
/// proportional to the corpus.
pub const ENGLISH: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with",
];

/// Drops tokens that appear in a word list.
///
/// Positions are preserved rather than renumbered, which is what keeps phrase
/// matching honest: with `the` removed from `"king of the hill"`, the gap it
/// leaves is why `"king of hill"` does not match it as an exact phrase.
#[derive(Debug, Clone)]
pub struct StopwordFilter {
    words: HashSet<String>,
}

impl StopwordFilter {
    /// A filter over an explicit word list.
    #[must_use]
    pub fn new<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            words: words
                .into_iter()
                .map(|w| w.as_ref().to_lowercase())
                .collect(),
        }
    }

    /// A filter over [`ENGLISH`].
    #[must_use]
    pub fn english() -> Self {
        Self::new(ENGLISH.iter().copied())
    }

    /// A filter that removes nothing.
    ///
    /// Useful for a field where every word matters, without restructuring the
    /// analyzer to omit the filter entirely.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            words: HashSet::new(),
        }
    }

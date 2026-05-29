//! Turning text into terms.
//!
//! The pipeline is a tokenizer followed by an ordered list of filters, which is
//! the arrangement every mature search engine converges on. The important
//! property is that indexing and querying run the *same* pipeline: if `Running`
//! is indexed as `run`, the query `Running` has to become `run` too, or the
//! document is unfindable. [`Analyzer`] is the single object both sides use so
//! they cannot drift.

pub mod ascii;
pub mod ngram;
pub mod standard;
pub mod stemmer;
pub mod stopwords;

use std::fmt::Debug;

/// One term, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The term text, after filtering.
    pub text: String,
    /// Ordinal within the field, counting from zero.
    ///
    /// Phrase matching compares these, so a filter that drops a token must
    /// leave a gap rather than renumber — otherwise removing a stopword would
    /// make `"king of pop"` match `"king pop"`.
    pub position: u32,
    /// Byte offset of the original text in the input.
    pub start: usize,
    /// Byte offset just past the original text.
    pub end: usize,
}

impl Token {
    /// A token covering the given span.
    #[must_use]
    pub fn new(text: impl Into<String>, position: u32, start: usize, end: usize) -> Self {
        Self {
            text: text.into(),
            position,
            start,
            end,
        }
    }

    /// The token's text replaced, keeping its position and span.
    #[must_use]
    pub fn with_text(&self, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: self.position,
            start: self.start,
            end: self.end,
        }
    }

    /// Length in bytes of the original text.
    #[must_use]
    pub fn span_len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Splits text into tokens.
pub trait Tokenizer: Debug + Send + Sync {
    /// Splits `input`.
    fn tokenize(&self, input: &str) -> Vec<Token>;
}

/// Transforms a token stream.
pub trait TokenFilter: Debug + Send + Sync {
    /// Rewrites, drops or expands tokens.
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token>;

    /// A short name, used when describing an analyzer in diagnostics.
    fn name(&self) -> &'static str;
}

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

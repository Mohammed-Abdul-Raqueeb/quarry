//! Snippet extraction.
//!
//! A result list is far more useful when it shows *why* a document matched.
//! That has to work from the stored original text rather than the index:
//! analysis is lossy, so the term `run` cannot be turned back into the word
//! `running` that the user needs to see highlighted.
//!
//! The approach is to re-analyze the stored text with the same analyzer, which
//! yields byte offsets for each term, then pick the densest window of matches.

use crate::tokenize::Analyzer;

/// How a snippet is built.
#[derive(Debug, Clone)]
pub struct HighlightOptions {
    /// Target snippet width in characters.
    pub width: usize,
    /// Inserted before a matched term.
    pub open: String,
    /// Inserted after a matched term.
    pub close: String,
    /// Inserted where text was cut.
    pub ellipsis: String,
    /// Most snippets to return per field.
    pub max_snippets: usize,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        Self {
            width: 160,
            open: "<mark>".to_string(),
            close: "</mark>".to_string(),
            ellipsis: "…".to_string(),
            max_snippets: 1,
        }
    }
}

impl HighlightOptions {
    /// Options that mark matches with `*asterisks*`, for terminal output.
    #[must_use]
    pub fn terminal() -> Self {
        Self {
            open: "\u{1b}[1;33m".to_string(),
            close: "\u{1b}[0m".to_string(),
            ..Self::default()
        }
    }

    /// Options that wrap matches in plain brackets, for tests and plain text.
    #[must_use]
    pub fn brackets() -> Self {
        Self {
            open: "[".to_string(),
            close: "]".to_string(),
            ..Self::default()
        }
    }
}

/// A byte range in the original text that matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start.
    pub start: usize,
    /// Exclusive end.
    pub end: usize,
}

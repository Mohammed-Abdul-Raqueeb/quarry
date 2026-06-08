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

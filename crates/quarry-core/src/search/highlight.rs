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

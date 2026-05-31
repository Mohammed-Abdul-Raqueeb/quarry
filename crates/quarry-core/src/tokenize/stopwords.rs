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

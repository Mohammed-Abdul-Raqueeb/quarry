//! The parsed shape of a query.

use std::fmt;

/// A query before it has been resolved against a schema.
///
/// This is a tree because that is what a user writes — `a AND (b OR c)` nests.
/// The planner flattens it into the three-clause boolean the executor runs,
/// which is a lossy transformation and the reason the two representations are
/// separate types rather than one.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// A single word.
    Term {
        /// Field name, or `None` to search the default fields.
        field: Option<String>,
        /// The word, unanalyzed.
        text: String,
    },
    /// A quoted sequence of words that must appear in order.
    Phrase {
        /// Field name, or `None` for the default fields.
        field: Option<String>,
        /// The words, unanalyzed.
        terms: Vec<String>,
        /// Positions of slack allowed between adjacent terms.
        slop: u32,
    },
    /// A word with a bounded number of edits allowed.
    Fuzzy {
        /// Field name, or `None` for the default fields.
        field: Option<String>,
        /// The word, unanalyzed.
        text: String,
        /// Maximum edit distance.
        distance: u8,
    },
    /// A word with a trailing wildcard.
    Prefix {
        /// Field name, or `None` for the default fields.
        field: Option<String>,
//! Query errors.
//!
//! These are separate from [`quarry_core::Error`] because they are *user*
//! errors, not engine errors. A malformed query is something to show the person
//! who typed it, with a position, so it carries structure a message string
//! could not.

use std::fmt;

/// Why a query could not be parsed or planned.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    /// A character that has no meaning where it appeared.
    Unexpected {
        /// What was found.
        found: String,
        /// Character offset in the query.
        position: usize,
    },
    /// Something was required but the query ended.
    UnexpectedEnd {
        /// What the parser was looking for.
        expected: String,
    },
    /// A `"` with no closing partner.
    UnterminatedQuote {
        /// Where the quote opened.
        position: usize,
    },
    /// A `(` with no closing partner, or the reverse.
    UnbalancedParens,
    /// The query exceeded the length limit.
    TooLong {
        /// The query's length.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// The query nested deeper than the parser allows.
    TooDeep {
        /// The limit.
        max: usize,
    },
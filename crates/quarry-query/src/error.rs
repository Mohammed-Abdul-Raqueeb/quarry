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
    /// A field in the query is not in the schema.
    UnknownField {
        /// The name that was not found.
        name: String,
        /// Field names that exist, for a suggestion.
        available: Vec<String>,
    },
    /// A range was used on a field that is not numeric.
    NotRangeable {
        /// The field named.
        field: String,
        /// The type it actually has.
        found: String,
    },
    /// A phrase was used on a field indexed without positions.
    NoPositions {
        /// The field named.
        field: String,
    },
    /// A fuzzy distance outside the supported range.
    BadFuzzyDistance {
        /// What was asked for.
        found: u32,
        /// The largest supported.
        max: u8,
    },
    /// A range whose bounds cross.
    EmptyRange {
        /// The field named.
        field: String,
    },
    /// The query had no searchable clauses.
    Empty,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unexpected { found, position } => {
                write!(f, "unexpected {found:?} at position {position}")
            }
            Self::UnexpectedEnd { expected } => {
                write!(f, "query ended while expecting {expected}")
            }
            Self::UnterminatedQuote { position } => {
                write!(f, "unterminated quote opened at position {position}")
            }
            Self::UnbalancedParens => f.write_str("unbalanced parentheses"),
            Self::TooLong { len, max } => {
                write!(f, "query is {len} bytes, over the {max} byte limit")
            }
            Self::TooDeep { max } => write!(f, "query nests deeper than {max} levels"),
            Self::UnknownField { name, available } => {
                write!(f, "no field named {name:?}")?;
                // Suggesting the closest name turns a typo into a one-step fix.
                if let Some(closest) = closest_match(name, available) {
                    write!(f, "; did you mean {closest:?}?")?;
                } else if !available.is_empty() {
                    write!(f, "; available fields: {}", available.join(", "))?;
                }
                Ok(())
            }
            Self::NotRangeable { field, found } => {
                write!(
                    f,
                    "field {field:?} is {found}, and only numeric fields support ranges"
                )
            }
            Self::NoPositions { field } => {
                write!(
                    f,
                    "field {field:?} was indexed without positions, so it cannot be phrase-matched"
                )
            }
            Self::BadFuzzyDistance { found, max } => {
                write!(f, "fuzzy distance {found} is above the maximum of {max}")
            }
            Self::EmptyRange { field } => {
                write!(f, "the range on {field:?} cannot match: its bounds cross")
            }
            Self::Empty => f.write_str("the query has no searchable terms"),
        }
    }
}

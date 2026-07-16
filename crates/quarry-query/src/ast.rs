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
        /// The prefix, unanalyzed.
        text: String,
    },
    /// A numeric range.
    Range {
        /// Field name. A range always names its field.
        field: String,
        /// Lower bound, if any.
        lower: Option<f64>,
        /// Upper bound, if any.
        upper: Option<f64>,
        /// Whether the lower bound is inclusive.
        include_lower: bool,
        /// Whether the upper bound is inclusive.
        include_upper: bool,
    },
    /// All of these must match.
    And(Vec<Query>),
    /// Any of these may match.
    Or(Vec<Query>),
    /// This must not match.
    Not(Box<Query>),
    /// A subquery whose score is multiplied.
    Boost {
        /// The subquery.
        query: Box<Query>,
        /// The multiplier.
        factor: f32,
    },
    /// Matches every document.
    MatchAll,
    /// Matches nothing.
    ///
    /// Produced when a clause is provably unsatisfiable, so the executor can
    /// return early rather than scanning.
    MatchNone,
}

impl Query {
    /// A bare term on the default fields.
    #[must_use]
    pub fn term(text: impl Into<String>) -> Self {
        Self::Term {
            field: None,
            text: text.into(),
        }
    }

    /// A term scoped to a field.
    #[must_use]
    pub fn field_term(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Term {
            field: Some(field.into()),
            text: text.into(),
        }
    }

    /// Wraps this query in a boost.
    #[must_use]
    pub fn boost(self, factor: f32) -> Self {
        Self::Boost {
            query: Box::new(self),
            factor,
        }
    }

    /// Negates this query.
    #[must_use]
    pub fn negate(self) -> Self {
        Self::Not(Box::new(self))
    }

    /// Whether the query can match anything at all.
    #[must_use]
    pub fn is_match_none(&self) -> bool {
        matches!(self, Self::MatchNone)
    }

    /// How deeply the query nests.
    ///
    /// The parser caps this; a deeply nested query is cheap to write and
    /// expensive to plan.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::And(children) | Self::Or(children) => {
                1 + children.iter().map(Self::depth).max().unwrap_or(0)
            }
            Self::Not(child) => 1 + child.depth(),
            Self::Boost { query, .. } => 1 + query.depth(),
            _ => 1,
        }
    }

    /// How many leaf clauses the query has.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::And(children) | Self::Or(children) => children.iter().map(Self::leaf_count).sum(),
            Self::Not(child) => child.leaf_count(),
            Self::Boost { query, .. } => query.leaf_count(),
            Self::MatchAll | Self::MatchNone => 0,
            _ => 1,
        }
    }

    /// Every field named anywhere in the query.
    #[must_use]
    pub fn referenced_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(&mut fields);
        fields.sort();
        fields.dedup();
        fields
    }

    fn collect_fields(&self, out: &mut Vec<String>) {
        match self {
            Self::Term { field, .. }
            | Self::Phrase { field, .. }
            | Self::Fuzzy { field, .. }
            | Self::Prefix { field, .. } => {
                if let Some(name) = field {
                    out.push(name.clone());
                }
            }
            Self::Range { field, .. } => out.push(field.clone()),
            Self::And(children) | Self::Or(children) => {
                for child in children {
                    child.collect_fields(out);
                }
            }
            Self::Not(child) => child.collect_fields(out),
            Self::Boost { query, .. } => query.collect_fields(out),
            Self::MatchAll | Self::MatchNone => {}
        }
    }

    /// Collapses redundant structure.
    ///
    /// Single-child boolean nodes, nested nodes of the same kind, and clauses
    /// adjacent to `MatchNone` all simplify. Worth doing because the parser
    /// naturally produces them and the planner is simpler if it never sees them.
    #[must_use]
    pub fn simplify(self) -> Self {
        match self {
            Self::And(children) => {
                let mut flat = Vec::new();
                for child in children {
                    match child.simplify() {
                        // `a AND (b AND c)` is `a AND b AND c`.
                        Self::And(inner) => flat.extend(inner),
                        // Anything AND nothing is nothing.
                        Self::MatchNone => return Self::MatchNone,
                        // MatchAll adds no constraint.
                        Self::MatchAll => {}
                        other => flat.push(other),
                    }
                }
                match flat.len() {
                    0 => Self::MatchAll,
                    1 => flat.remove(0),
                    _ => Self::And(flat),
                }
            }
            Self::Or(children) => {
                let mut flat = Vec::new();
                for child in children {
                    match child.simplify() {
                        Self::Or(inner) => flat.extend(inner),
                        // Anything OR everything is everything.
                        Self::MatchAll => return Self::MatchAll,
                        Self::MatchNone => {}
                        other => flat.push(other),
                    }
                }
                match flat.len() {
                    0 => Self::MatchNone,
                    1 => flat.remove(0),
                    _ => Self::Or(flat),
                }
            }
            Self::Not(child) => match child.simplify() {
                Self::MatchNone => Self::MatchAll,
                Self::MatchAll => Self::MatchNone,
                // Double negation cancels.
                Self::Not(inner) => *inner,
                other => Self::Not(Box::new(other)),
            },
            Self::Boost { query, factor } => {
                // A boost of exactly 1 changes nothing.
                if (factor - 1.0).abs() < f32::EPSILON {
                    query.simplify()
                } else {
                    Self::Boost {
                        query: Box::new(query.simplify()),
                        factor,
                    }
                }
            }
            other => other,
        }
    }
}

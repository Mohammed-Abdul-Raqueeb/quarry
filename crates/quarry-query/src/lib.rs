//! The Quarry query language.
//!
//! Text goes through three stages, each a separate module because each can fail
//! for a different reason and the user needs to be told which:
//!
//! ```text
//! "title:rust~1 AND NOT go"
//!        │
//!   lexer│  tokens
//!        ▼
//!  parser│  Query (a tree, as written)
//!        ▼
//! planner│  BooleanQuery (flat, as executed)
//!        ▼
//!   quarry_core::Searcher
//! ```
//!
//! The planner is where a query stops being syntax and starts depending on the
//! index: it resolves field names, runs the query text through the same
//! analyzer the documents used, and expands fuzzy and prefix clauses against
//! the real dictionary.
//!
//! # Example
//!
//! ```
//! use quarry_query::{parse, Query};
//!
//! let query = parse("title:rust AND NOT deprecated")?;
//! assert_eq!(query.referenced_fields(), vec!["title".to_string()]);
//! assert_eq!(query.leaf_count(), 2);
//! # Ok::<(), quarry_query::QueryError>(())
//! ```

pub mod ast;
pub mod error;
pub mod fuzzy;
pub mod lexer;
pub mod parser;
pub mod planner;

pub use ast::Query;
pub use error::{QueryError, Result};
pub use fuzzy::{bounded_levenshtein, damerau_levenshtein, levenshtein, similarity, FuzzyMatch};
pub use lexer::{tokenize, Tok};
pub use parser::parse;
pub use planner::{plan, Plan, PlanContext};

/// The version of this crate, from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

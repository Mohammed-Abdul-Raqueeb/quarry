//! Turning an AST into something the executor can run.
//!
//! Three jobs, none of which the parser can do because none of them are
//! syntactic:
//!
//! - **Resolve fields.** A bare term searches every default field, so one
//!   clause becomes several. An unknown field is caught here, with a
//!   suggestion.
//! - **Analyze terms.** The query text has to go through the same analyzer the
//!   documents did, or `Running` will not find the indexed `run`. A term that
//!   analyzes away to nothing — a stopword — is dropped.
//! - **Expand.** Fuzzy and prefix clauses become the set of real dictionary
//!   terms they match, since the executor only knows how to look up exact
//!   terms.
//!
//! The output is the flat three-clause boolean [`quarry_core::BooleanQuery`],
//! which is lossy: nested alternation collapses into `should`. That is a
//! deliberate limit, and `docs/query-language.md` says so.

use crate::ast::Query;
use crate::error::{QueryError, Result};
use crate::fuzzy;
use quarry_core::index::Segment;
use quarry_core::schema::{FieldId, FieldType, Schema};
use quarry_core::search::{BooleanQuery, PhraseQuery, TermQuery};
use quarry_core::tokenize::Analyzer;

/// Most dictionary terms one fuzzy or prefix clause may expand to.
///
/// A prefix like `a*` can match a large fraction of the dictionary; without a
/// cap, one character of input turns into a query with thousands of clauses.
pub const MAX_EXPANSIONS: usize = 64;

/// What the planner needs to resolve a query.
pub struct PlanContext<'a> {
    /// Fields and their types.
    pub schema: &'a Schema,
    /// The analyzer the documents were indexed with.
    pub analyzer: &'a Analyzer,
    /// Segments, for expanding fuzzy and prefix clauses against the dictionary.
    pub segments: &'a [Segment],
}

impl std::fmt::Debug for PlanContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanContext")
            .field("fields", &self.schema.len())
            .field("segments", &self.segments.len())
            .finish()
    }
}

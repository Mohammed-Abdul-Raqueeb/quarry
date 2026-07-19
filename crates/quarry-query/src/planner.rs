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

/// A planned query plus what the planner did to it.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The executable query.
    pub query: BooleanQuery,
    /// Notes worth showing the user.
    ///
    /// Dropped stopwords and truncated expansions both silently change what a
    /// query means, so they are reported rather than hidden.
    pub notes: Vec<String>,
}

impl Plan {
    /// Whether the plan can match anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

/// Plans a parsed query.
pub fn plan(query: &Query, ctx: &PlanContext<'_>) -> Result<Plan> {
    let mut boolean = BooleanQuery::new();
    let mut notes = Vec::new();

    let mut resolved_any = false;
    walk(
        query,
        ctx,
        Occur::Should,
        1.0,
        &mut boolean,
        &mut notes,
        &mut resolved_any,
    )?;

    // Two different ways to end up with no clauses, and they deserve different
    // answers.
    //
    // Nothing resolved at all — a query of pure stopwords, or one with no
    // leaves — is a user error, and saying so beats silently returning
    // nothing. But a term that resolved and simply matched no dictionary
    // entry, like a prefix nobody has indexed, is an ordinary empty result:
    // the query was well formed, the corpus just does not contain it.
    if boolean.is_empty() && !resolved_any {
        return Err(QueryError::Empty);
    }

    // With no required clauses, demand at least one optional match so an
    // implicit OR does not degenerate into matching every document.
    if boolean.must.is_empty()
        && boolean.must_any.is_empty()
        && boolean.phrases.is_empty()
        && !boolean.should.is_empty()
    {
        boolean.minimum_should_match = 1;
    }

    Ok(Plan {
        query: boolean,
        notes,
    })
}

/// Where a clause lands in the flat boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Occur {
    Must,
    Should,
    MustNot,
}

impl Occur {
    /// Flips `must` and `must_not` under a negation.
    ///
    /// `should` also becomes `must_not`: inside a `NOT`, matching an optional
    /// clause is what disqualifies a document.
    fn negate(self) -> Self {
        match self {
            Self::Must | Self::Should => Self::MustNot,
            Self::MustNot => Self::Must,
        }
    }
}

fn walk(
    query: &Query,
    ctx: &PlanContext<'_>,
    occur: Occur,
    boost: f32,
    out: &mut BooleanQuery,
    notes: &mut Vec<String>,
    // Set once any leaf resolved to a real term, which separates "nothing
    // matched" from "nothing to search for".
    resolved: &mut bool,
) -> Result<()> {
    match query {
        Query::MatchAll | Query::MatchNone => {
            // Neither has a term to look up. MatchAll is handled by the caller
            // seeing an empty query; MatchNone contributes nothing.
            Ok(())
        }

        Query::Term { field, text } => {
            let fields = resolve_fields(field.as_deref(), ctx)?;
            let terms = analyze(text, ctx, &fields);

            if terms.is_empty() {
                notes.push(format!("{text:?} analyzed to nothing and was dropped"));
                return Ok(());
            }

            *resolved = true;
            push_expansion(out, occur, terms, boost);
            Ok(())
        }

        Query::Prefix { field, text } => {
            let fields = resolve_fields(field.as_deref(), ctx)?;
            // Lowercased rather than fully analyzed: stemming a prefix would
            // change the characters being matched against.
            let prefix = text.to_lowercase();
            // The prefix itself resolved, whether or not it expands to
            // anything in this corpus.
            *resolved = true;

            let mut total = 0;
            for field_id in fields {
                let expansions = expand_prefix(ctx, field_id, &prefix);
                total += expansions.len();

                for term in expansions {
                    push(out, occur, TermQuery::new(field_id, term).boosted(boost));
                }
            }

            if total == 0 {
                notes.push(format!("no terms start with {prefix:?}"));
            } else if total >= MAX_EXPANSIONS {
                notes.push(format!(
                    "prefix {prefix:?} matched more than {MAX_EXPANSIONS} terms and was truncated"
                ));
            }
            Ok(())
        }

        Query::Fuzzy {
            field,
            text,
            distance,
        } => {
            let fields = resolve_fields(field.as_deref(), ctx)?;
            let analyzed = analyze(text, ctx, &fields);

            if analyzed.is_empty() {
                notes.push(format!("{text:?} analyzed to nothing and was dropped"));
                return Ok(());
            }

            *resolved = true;

            for (field_id, term) in analyzed {
                let matches = expand_fuzzy(ctx, field_id, &term, *distance as usize);

                if matches.is_empty() {
                    notes.push(format!("nothing within {distance} edits of {term:?}"));
                    continue;
                }

                for found in matches {
                    // Closer matches score higher, so an exact hit outranks a
                    // two-edit neighbour.
                    let decay = 1.0 / (1.0 + found.distance as f32);
                    push(
                        out,
                        occur,
                        TermQuery::new(field_id, found.term).boosted(boost * decay),
                    );
                }
            }
            Ok(())
        }

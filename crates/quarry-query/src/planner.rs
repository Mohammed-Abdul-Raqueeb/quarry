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
use quarry_core::search::{BooleanQuery, PhraseQuery, RangeQuery, TermQuery};
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

        Query::Phrase { field, terms, slop } => {
            let fields = resolve_fields(field.as_deref(), ctx)?;

            for field_id in fields {
                let definition =
                    ctx.schema
                        .field_by_id(field_id)
                        .ok_or_else(|| QueryError::UnknownField {
                            name: field_id.to_string(),
                            available: field_names(ctx.schema),
                        })?;

                if !definition.options.positions {
                    return Err(QueryError::NoPositions {
                        field: definition.name.clone(),
                    });
                }

                // Each word analyzed separately, so the phrase keeps one term
                // per input word even if one of them analyzes to nothing.
                let analyzed: Vec<String> = terms
                    .iter()
                    .filter_map(|word| ctx.analyzer.terms(word).into_iter().next())
                    .collect();

                if analyzed.len() < 2 {
                    notes.push(format!(
                        "phrase {terms:?} analyzed to fewer than two terms; matching as terms instead"
                    ));
                    for term in analyzed {
                        push(out, occur, TermQuery::new(field_id, term).boosted(boost));
                    }
                    continue;
                }

                *resolved = true;
                out.phrases
                    .push(PhraseQuery::new(field_id, analyzed).with_slop(*slop));
            }
            Ok(())
        }

        Query::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let definition = ctx
                .schema
                .field(field)
                .ok_or_else(|| QueryError::UnknownField {
                    name: field.clone(),
                    available: field_names(ctx.schema),
                })?;

            if definition.ty != FieldType::Numeric {
                return Err(QueryError::NotRangeable {
                    field: field.clone(),
                    found: definition.ty.as_str().to_string(),
                });
            }

            let planned = RangeQuery {
                field: definition.id,
                lower: *lower,
                upper: *upper,
                include_lower: *include_lower,
                include_upper: *include_upper,
            };

            // The parser already rejects crossed bounds, but equal bounds with
            // one end excluded also describe an empty interval and reach here.
            if planned.is_empty() {
                return Err(QueryError::EmptyRange {
                    field: field.clone(),
                });
            }

            *resolved = true;

            // A negated range would need the executor to subtract a set it
            // currently only intersects. Reported rather than silently dropped,
            // which would turn an exclusion into no filter at all.
            if occur == Occur::MustNot {
                notes.push(format!(
                    "a negated range on {field:?} is not supported and was ignored"
                ));
                return Ok(());
            }

            out.ranges.push(planned);
            Ok(())
        }

        Query::And(children) => {
            for child in children {
                // Inside an AND, an optional clause becomes required.
                let child_occur = if occur == Occur::Should {
                    Occur::Must
                } else {
                    occur
                };
                walk(child, ctx, child_occur, boost, out, notes, resolved)?;
            }
            Ok(())
        }

        Query::Or(children) => {
            for child in children {
                walk(child, ctx, Occur::Should, boost, out, notes, resolved)?;
            }
            Ok(())
        }

        Query::Not(child) => walk(child, ctx, occur.negate(), boost, out, notes, resolved),

        Query::Boost { query, factor } => {
            walk(query, ctx, occur, boost * factor, out, notes, resolved)
        }
    }
}

/// Places a term that expanded across several fields.
///
/// Under `must`, the expansion is a disjunction: the word has to appear in one
/// of the default fields, not in every one of them. Pushing each expansion as
/// its own `must` clause would instead demand that a document repeat the word
/// in title and body alike — a bug that surfaces as a query returning nothing
/// rather than as an error, which is why it gets its own function.
fn push_expansion(out: &mut BooleanQuery, occur: Occur, terms: Vec<(FieldId, String)>, boost: f32) {
    if occur == Occur::Must && terms.len() > 1 {
        out.must_any.push(
            terms
                .into_iter()
                .map(|(field, term)| TermQuery::new(field, term).boosted(boost))
                .collect(),
        );
        return;
    }

    for (field, term) in terms {
        push(out, occur, TermQuery::new(field, term).boosted(boost));
    }
}

fn push(out: &mut BooleanQuery, occur: Occur, term: TermQuery) {
    match occur {
        Occur::Must => out.must.push(term),
        Occur::Should => out.should.push(term),
        Occur::MustNot => out.must_not.push(term),
    }
}

/// The field ids a clause applies to.
fn resolve_fields(field: Option<&str>, ctx: &PlanContext<'_>) -> Result<Vec<FieldId>> {
    if let Some(name) = field {
        let definition = ctx
            .schema
            .field(name)
            .ok_or_else(|| QueryError::UnknownField {
                name: name.to_string(),
                available: field_names(ctx.schema),
            })?;
        Ok(vec![definition.id])
    } else {
        let fields = ctx.schema.default_search_fields();
        if fields.is_empty() {
            // Nothing to search means every bare term would be dropped,
            // which is worth an error rather than empty results.
            return Err(QueryError::Empty);
        }
        Ok(fields)
    }
}

/// Analyzes a term for each field, respecting that field's type.
///
/// A keyword field is lowercased but not stemmed, so `Rust-Lang` stays one
/// term; a text field goes through the full pipeline.
fn analyze(text: &str, ctx: &PlanContext<'_>, fields: &[FieldId]) -> Vec<(FieldId, String)> {
    let mut out = Vec::new();

    for &field_id in fields {
        let Some(definition) = ctx.schema.field_by_id(field_id) else {
            continue;
        };

        match definition.ty {
            FieldType::Text => {
                for term in ctx.analyzer.terms(text) {
                    out.push((field_id, term));
                }
            }
            FieldType::Keyword | FieldType::Numeric => {
                out.push((field_id, text.to_lowercase()));
            }
        }
    }

    out
}

/// Dictionary terms in `field` starting with `prefix`.
fn expand_prefix(ctx: &PlanContext<'_>, field: FieldId, prefix: &str) -> Vec<String> {
    let mut terms = Vec::new();

    for segment in ctx.segments {
        for term in segment.index.terms_with_prefix(field, prefix) {
            terms.push(term.to_string());
            if terms.len() >= MAX_EXPANSIONS {
                break;
            }
        }
    }

    // The same term can appear in several segments.
    terms.sort();
    terms.dedup();
    terms.truncate(MAX_EXPANSIONS);
    terms
}

/// Dictionary terms in `field` within `distance` edits of `term`.
fn expand_fuzzy(
    ctx: &PlanContext<'_>,
    field: FieldId,
    term: &str,
    distance: usize,
) -> Vec<fuzzy::FuzzyMatch> {
    if distance == 0 {
        return vec![fuzzy::FuzzyMatch {
            term: term.to_string(),
            distance: 0,
        }];
    }

    // Requiring a shared first character turns a full dictionary scan into a
    // range scan. Typos in the first letter are rare enough that the recall
    // lost is worth the cost saved.
    let prefix_len = usize::from(term.chars().count() > 3);

    let mut all: Vec<fuzzy::FuzzyMatch> = Vec::new();

    for segment in ctx.segments {
        let candidates = segment.index.terms_in_field(field);
        all.extend(fuzzy::find_matches(term, &candidates, distance, prefix_len));
    }

    all.sort_by(|a, b| {
        a.distance
            .cmp(&b.distance)
            .then_with(|| a.term.cmp(&b.term))
    });
    all.dedup_by(|a, b| a.term == b.term);
    all.truncate(MAX_EXPANSIONS);
    all
}

fn field_names(schema: &Schema) -> Vec<String> {
    schema.fields().iter().map(|f| f.name.clone()).collect()
}

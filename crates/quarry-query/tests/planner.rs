//! Resolving a parsed query against a real index.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::tokenize::Analyzer;
use quarry_core::{Document, IndexWriter, Value};
use quarry_query::planner::MAX_EXPANSIONS;
use quarry_query::{parse, plan, PlanContext, QueryError};
use std::sync::OnceLock;

/// Shared schema, held in a `OnceLock` so `PlanContext` can borrow it.
fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Schema::builder()
            .text("title")
            .and_then(|b| b.text("body"))
            .and_then(|b| b.keyword("tag"))
            .and_then(|b| b.numeric("year"))
            .and_then(SchemaBuilder::build)
            .expect("schema should build")
    })
}

fn analyzer() -> &'static Analyzer {
    static ANALYZER: OnceLock<Analyzer> = OnceLock::new();
    ANALYZER.get_or_init(Analyzer::english)
}

fn segments() -> Vec<Segment> {
    let mut writer = IndexWriter::new(schema().clone());

    for (title, body, tag) in [
        ("Rust ownership", "borrowing and the borrow checker", "rust"),
        ("Zero cost abstractions", "iterators compile away", "rust"),
        ("Python typing", "gradual annotations", "python"),
    ] {
        let mut doc = Document::new();
        doc.add(0, Value::Text(title.to_string()));
        doc.add(1, Value::Text(body.to_string()));
        doc.add(2, Value::Keyword(tag.to_string()));
        writer.add(&doc).expect("should index");
    }

    writer.flush().map(|s| vec![s]).unwrap_or_default()
}

/// Plans `input`, panicking with the reason if it cannot be planned.
fn planned(input: &str, segments: &[Segment]) -> quarry_query::Plan {
    let ctx = PlanContext {
        schema: schema(),
        analyzer: analyzer(),
        segments,
    };
    let ast = parse(input).unwrap_or_else(|err| panic!("{input:?} should parse: {err}"));
    plan(&ast, &ctx).unwrap_or_else(|err| panic!("{input:?} should plan: {err}"))
}

fn plan_error(input: &str, segments: &[Segment]) -> QueryError {
    let ctx = PlanContext {
        schema: schema(),
        analyzer: analyzer(),
        segments,
    };
    let ast = parse(input).expect("should parse");
    plan(&ast, &ctx).expect_err("should have failed to plan")
}

// ---- field resolution ---------------------------------------------------

#[test]
fn a_field_scoped_term_resolves_to_that_field() {
    let segments = segments();
    let plan = planned("title:rust", &segments);

    assert_eq!(plan.query.should.len(), 1);
    assert_eq!(plan.query.should[0].field, 0);
}

#[test]
fn a_bare_term_expands_across_the_default_fields() {
    let segments = segments();
    let plan = planned("rust", &segments);

    // Title and body, but not the keyword or numeric fields.
    assert_eq!(plan.query.should.len(), 2);
}

#[test]
fn an_unknown_field_is_an_error() {
    let segments = segments();
    assert!(matches!(
        plan_error("nope:rust", &segments),
        QueryError::UnknownField { .. }
    ));
}

#[test]
fn an_unknown_field_error_suggests_the_closest_name() {
    // A typo should be a one-step fix, not a hunt through the schema.
    let segments = segments();
    let message = plan_error("titel:rust", &segments).to_string();

    assert!(message.contains("did you mean"), "message was: {message}");
    assert!(message.contains("title"), "message was: {message}");
}

#[test]
fn an_unrelated_field_name_gets_no_suggestion() {
    // Suggesting a field that merely shares a letter sends the user the wrong
    // way, which is worse than suggesting nothing.
    let segments = segments();
    let message = plan_error("kubernetes:rust", &segments).to_string();

    assert!(!message.contains("did you mean"), "message was: {message}");
}

// ---- analysis -----------------------------------------------------------

#[test]
fn a_query_term_is_analyzed_the_way_documents_were() {
    // Without this, a search for `Running` cannot find the indexed `run`.
    let segments = segments();
    let plan = planned("title:BORROWING", &segments);

    assert_eq!(plan.query.should[0].term, "borrow");
}

#[test]
fn a_keyword_field_term_is_lowercased_but_not_stemmed() {
    let segments = segments();
    let plan = planned("tag:RUST", &segments);

    assert_eq!(plan.query.should[0].term, "rust");
}

#[test]
fn a_stopword_only_query_is_rejected_rather_than_returning_nothing() {
    // Returning zero results with no reason is the confusing alternative.
    let segments = segments();
    assert_eq!(plan_error("the", &segments), QueryError::Empty);
}

#[test]
fn a_dropped_stopword_is_reported_as_a_note() {
    let segments = segments();
    let plan = planned("the AND rust", &segments);

    assert!(
        plan.notes.iter().any(|n| n.contains("analyzed to nothing")),
        "notes were {:?}",
        plan.notes
    );
}

// ---- boolean placement --------------------------------------------------

#[test]
fn an_and_makes_its_clauses_required() {
    let segments = segments();
    let plan = planned("title:rust AND title:ownership", &segments);

    assert_eq!(plan.query.must.len(), 2);
    assert!(plan.query.should.is_empty());
}

#[test]
fn an_or_leaves_its_clauses_optional() {
    let segments = segments();
    let plan = planned("title:rust OR title:python", &segments);

    assert_eq!(plan.query.should.len(), 2);
    assert!(plan.query.must.is_empty());
}

#[test]
fn a_negation_becomes_a_must_not_clause() {
    let segments = segments();
    let plan = planned("title:rust AND NOT title:python", &segments);

    assert_eq!(plan.query.must.len(), 1);
    assert_eq!(plan.query.must_not.len(), 1);
}

#[test]
fn a_required_multi_field_term_becomes_one_disjunctive_group() {
    // The word must appear in one of title or body, not in both. Pushing each
    // expansion as its own `must` is the bug this guards against.
    let segments = segments();
    let plan = planned("rust AND ownership", &segments);

    assert_eq!(plan.query.must_any.len(), 2);
    assert!(plan.query.must.is_empty());
    assert_eq!(plan.query.must_any[0].len(), 2);
}

#[test]
fn a_required_single_field_term_stays_an_ordinary_must() {
    let segments = segments();
    let plan = planned("title:rust AND title:ownership", &segments);

    assert!(plan.query.must_any.is_empty());
}

#[test]
fn an_optional_multi_field_term_spreads_across_should() {
    let segments = segments();
    let plan = planned("rust OR python", &segments);

    assert_eq!(plan.query.should.len(), 4);
    assert!(plan.query.must_any.is_empty());
}

#[test]
fn a_bare_disjunction_requires_at_least_one_match() {
    // Otherwise an implicit OR degenerates into matching every document.
    let segments = segments();
    let plan = planned("rust python", &segments);

    assert_eq!(plan.query.minimum_should_match, 1);
}

#[test]
fn a_query_with_required_clauses_sets_no_should_minimum() {
    let segments = segments();
    let plan = planned("title:rust AND title:ownership", &segments);

    assert_eq!(plan.query.minimum_should_match, 0);
}

// ---- boosts -------------------------------------------------------------

#[test]
fn a_boost_reaches_the_term_query() {
    let segments = segments();
    let plan = planned("title:rust^3", &segments);

    assert_eq!(plan.query.should[0].boost, 3.0);
}

#[test]
fn a_boost_on_a_group_reaches_every_leaf() {
    let segments = segments();
    let plan = planned("(title:rust OR title:python)^2", &segments);

    assert!(plan.query.should.iter().all(|t| t.boost == 2.0));
}

#[test]
fn nested_boosts_multiply() {
    let segments = segments();
    let plan = planned("((title:rust)^2)^3", &segments);

    assert_eq!(plan.query.should[0].boost, 6.0);
}

// ---- phrases ------------------------------------------------------------

#[test]
fn a_phrase_becomes_a_phrase_clause() {
    let segments = segments();
    let plan = planned("title:\"zero cost\"", &segments);

    assert_eq!(plan.query.phrases.len(), 1);
    assert_eq!(plan.query.phrases[0].terms.len(), 2);
}

#[test]
fn a_phrase_keeps_its_slop() {
    let segments = segments();
    let plan = planned("title:\"zero cost\"~3", &segments);

    assert_eq!(plan.query.phrases[0].slop, 3);
}

#[test]
fn a_phrase_on_a_field_without_positions_is_an_error() {
    let segments = segments();
    assert!(matches!(
        plan_error("tag:\"a b\"", &segments),
        QueryError::NoPositions { .. }
    ));
}

#[test]
fn a_phrase_whose_terms_mostly_analyze_away_falls_back_to_terms() {
    // `and` is a stopword, so only one term survives; demanding a phrase of
    // one term would need positions for no benefit.
    let segments = segments();
    let plan = planned("title:\"the rust\"", &segments);

    assert!(plan.query.phrases.is_empty());
    assert!(!plan.notes.is_empty());
}

// ---- prefix expansion ---------------------------------------------------

#[test]
fn a_prefix_expands_to_real_dictionary_terms() {
    let segments = segments();
    let plan = planned("title:own*", &segments);

    assert!(!plan.query.should.is_empty());
    assert!(plan.query.should.iter().all(|t| t.term.starts_with("own")));
}

#[test]
fn an_unmatched_prefix_is_reported_as_a_note() {
    let segments = segments();
    let plan = planned("title:kuber*", &segments);

    assert!(plan.query.should.is_empty());
    assert!(plan.notes.iter().any(|n| n.contains("no terms start with")));
}

#[test]
fn prefix_expansion_is_capped() {
    // One character of input must not become thousands of clauses.
    let mut writer = IndexWriter::new(schema().clone());
    for i in 0..(MAX_EXPANSIONS * 2) {
        let mut doc = Document::new();
        doc.add(0, Value::Text(format!("prefix{i:04}")));
        writer.add(&doc).expect("should index");
    }
    let segments = writer.flush().map(|s| vec![s]).unwrap_or_default();

    let plan = planned("title:prefix*", &segments);
    assert!(plan.query.should.len() <= MAX_EXPANSIONS);
}

#[test]
fn a_truncated_prefix_expansion_is_reported() {
    let mut writer = IndexWriter::new(schema().clone());
    for i in 0..(MAX_EXPANSIONS * 2) {
        let mut doc = Document::new();
        doc.add(0, Value::Text(format!("prefix{i:04}")));
        writer.add(&doc).expect("should index");
    }
    let segments = writer.flush().map(|s| vec![s]).unwrap_or_default();

    let plan = planned("title:prefix*", &segments);
    assert!(plan.notes.iter().any(|n| n.contains("truncated")));
}

// ---- fuzzy expansion ----------------------------------------------------

#[test]
fn a_fuzzy_term_expands_to_nearby_dictionary_terms() {
    let segments = segments();
    let plan = planned("title:rust~1", &segments);

    assert!(plan.query.should.iter().any(|t| t.term == "rust"));
}

#[test]
fn a_zero_distance_fuzzy_term_is_just_the_term() {
    let segments = segments();
    let plan = planned("title:rust~0", &segments);

    assert_eq!(plan.query.should.len(), 1);
    assert_eq!(plan.query.should[0].term, "rust");
}

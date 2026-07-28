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

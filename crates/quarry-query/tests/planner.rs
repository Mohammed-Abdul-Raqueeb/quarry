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

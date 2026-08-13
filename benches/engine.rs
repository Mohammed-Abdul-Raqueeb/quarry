//! Indexing and query benchmarks.
//!
//! Written against `std::time` rather than Criterion, because Criterion is a
//! dependency and the point of a benchmark here is a number you can reproduce,
//! not a statistical model of it. Each case runs a warmup, then enough
//! iterations to take a measurable amount of time, and reports the median plus
//! the spread so an outlier is visible rather than averaged away.
//!
//!   cargo bench

use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::tokenize::Analyzer;
use quarry_core::{
    BooleanQuery, Document, IndexWriter, PhraseQuery, Searcher, Segment, TermQuery, Value,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Words assembled into synthetic documents.
///
/// A fixed vocabulary with a skewed distribution, because a uniform one makes
/// every term equally selective and hides the difference between a common and
/// a rare term — which is most of what query performance depends on.
const VOCABULARY: &[&str] = &[
    "rust",
    "ownership",
    "borrow",
    "checker",
    "compile",
    "time",
    "memory",
    "safety",
    "thread",
    "concurrency",
    "iterator",
    "abstraction",
    "zero",
    "cost",
    "trait",
    "lifetime",
    "reference",
    "mutable",
    "immutable",
    "allocation",
    "stack",
    "heap",
    "pointer",
    "slice",
    "vector",
    "string",
    "closure",
    "generic",
    "monomorphise",
    "inline",
    "optimise",
    "benchmark",
    "profile",
    "latency",
    "throughput",
    "index",
    "segment",
    "posting",
    "dictionary",
    "term",
    "frequency",
    "document",
    "query",
    "parser",
    "planner",
    "scorer",
    "collector",
    "snippet",
    "highlight",
    "merge",
];

fn schema() -> Schema {
    Schema::builder()
        .text("title")
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("schema should build")
}

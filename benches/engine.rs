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

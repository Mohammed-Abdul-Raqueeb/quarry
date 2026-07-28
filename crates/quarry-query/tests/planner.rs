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

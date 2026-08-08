//! The API endpoints.
//!
//! ```text
//! GET  /health              liveness, never touches the index
//! GET  /stats               what the index contains
//! GET  /schema              the field definitions
//! GET  /search?q=…&limit=…  run a query
//! POST /search              run a query from a JSON body
//! ```
//!
//! The server is read-only on purpose. Indexing holds a write buffer and flushes
//! segments, which is a lifecycle that does not belong behind an HTTP handler
//! that may be serving a hundred concurrent readers; `quarry index` owns it
//! instead, and `docs/server.md` says so.

use crate::http::{escape_json_string, Request, Response};
use quarry_core::json::{self, Json, JsonBuilder};
use quarry_core::search::HighlightOptions;
use quarry_core::tokenize::Analyzer;
use quarry_core::{IndexDir, IndexStats, Searcher};
use quarry_query::{parse, plan, PlanContext};
use std::time::Instant;

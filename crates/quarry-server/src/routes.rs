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

/// Largest `limit` a client may ask for.
///
/// Without a cap, `?limit=100000000` asks the collector for a heap of that
/// size before a single document is scored.
pub const MAX_LIMIT: usize = 200;

/// Everything a handler needs.
pub struct AppState {
    /// The index being served.
    pub index: IndexDir,
    /// The analyzer the index was built with.
    pub analyzer: Analyzer,
    /// When the process started, for the uptime field.
    pub started: Instant,
}

// The analyzer and start time are deliberately omitted: a debug dump of an
// analyzer is pages of filter structs and tells you nothing about the state.
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("docs", &self.index.doc_count())
            .field("segments", &self.index.segments().len())
            .finish()
    }
}

/// Dispatches a request.
#[must_use]
pub fn route(request: &Request, state: &AppState) -> Response {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => health(state),
        ("GET", "/stats") => stats(state),
        ("GET", "/schema") => schema(state),
        ("GET", "/search") => {
            let query = request.param("q").unwrap_or("").to_string();
            search(&query, request.param_num("limit", 10), request, state)
        }
        ("POST", "/search") => post_search(request, state),

        // A known path with the wrong method deserves a 405, not a 404: the
        // distinction tells a client whether to fix the URL or the verb.
        (_, "/health" | "/stats" | "/schema") => {
            Response::error(405, "this endpoint only accepts GET")
        }
        (_, "/search") => Response::error(405, "/search accepts GET or POST"),

        _ => Response::error(404, "no such endpoint; try /health, /stats or /search"),
    }
}

fn health(state: &AppState) -> Response {
    let body = JsonBuilder::new()
        .str("status", "ok")
        .str("version", quarry_core::VERSION)
        .num("uptime_seconds", state.started.elapsed().as_secs() as f64)
        .build();

    Response::json(200, body.to_string())
}

fn stats(state: &AppState) -> Response {
    let stats = IndexStats::collect(state.index.segments(), state.index.schema());

    let body = JsonBuilder::new()
        .num("documents", f64::from(stats.docs))
        .num("segments", stats.segments as f64)
        .num("terms", stats.terms as f64)
        .num("bytes", stats.bytes as f64)
        .num("fields", stats.fields as f64)
        .num("bytes_per_doc", stats.bytes_per_doc())
        .bool("should_merge", stats.should_merge())
        .build();

    Response::json(200, body.to_string())
}

fn schema(state: &AppState) -> Response {
    Response::json(
        200,
        quarry_core::dir::schema_to_json(state.index.schema()).to_string(),
    )
}

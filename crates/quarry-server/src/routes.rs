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

/// Reads `q` and `limit` from a JSON body.
fn post_search(request: &Request, state: &AppState) -> Response {
    let parsed = match json::parse(&request.body) {
        Ok(parsed) => parsed,
        Err(err) => return Response::error(400, &format!("body is not valid JSON: {err}")),
    };

    let Some(query) = parsed.get("q").and_then(Json::as_str) else {
        return Response::error(422, "the body needs a string \"q\" field");
    };

    let limit = parsed
        .get("limit")
        .and_then(Json::as_f64)
        .map_or(10, |n| n.max(0.0) as usize);

    search(query, limit, request, state)
}

fn search(query_text: &str, limit: usize, request: &Request, state: &AppState) -> Response {
    if query_text.trim().is_empty() {
        return Response::error(422, "a query is required; pass ?q=…");
    }

    let limit = limit.clamp(1, MAX_LIMIT);

    let ast = match parse(query_text) {
        Ok(ast) => ast,
        // A malformed query is the client's mistake, so it gets a 400 and the
        // parser's message, which names the position.
        Err(err) => return Response::error(400, &err.to_string()),
    };

    let ctx = PlanContext {
        schema: state.index.schema(),
        analyzer: &state.analyzer,
        segments: state.index.segments(),
    };

    let planned = match plan(&ast, &ctx) {
        Ok(planned) => planned,
        Err(err) => return Response::error(422, &err.to_string()),
    };

    let searcher = Searcher::new(state.index.segments(), state.index.schema());

    let results = match searcher.search(&planned.query, limit) {
        Ok(results) => results,
        Err(err) => return Response::error(500, &err.to_string()),
    };

    let terms = planned.query.all_terms();
    let highlight = request.param_flag("highlight");
    let options = HighlightOptions::default();

    let hits: Vec<Json> = results
        .hits
        .iter()
        .map(|hit| {
            let document = searcher
                .document(hit)
                .map_or(Json::Null, |doc| doc.to_json(state.index.schema()));

            let mut builder = JsonBuilder::new()
                .num("doc", f64::from(hit.doc))
                .num("score", f64::from(hit.score))
                .set("document", document);

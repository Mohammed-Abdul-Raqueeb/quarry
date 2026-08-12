//! HTTP parsing, routing and the thread pool.

use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::tokenize::Analyzer;
use quarry_core::{Document, IndexDir, IndexWriter, Value};
use quarry_server::http::{
    escape_json_string, parse_query, percent_decode, reason_phrase, Request, Response, MAX_BODY,
};
use quarry_server::routes::{route, AppState, MAX_LIMIT};
use quarry_server::ThreadPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quarry-server-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn schema() -> Schema {
    Schema::builder()
        .text("title")
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("schema should build")
}

/// An app state over a small throwaway index.
fn state(temp: &TempDir) -> AppState {
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    let mut writer = IndexWriter::new(schema());

    for (title, body, tag) in [
        ("Rust ownership", "the borrow checker enforces it", "rust"),
        ("Python typing", "gradual annotations", "python"),
    ] {
        let mut doc = Document::new();
        doc.add(0, Value::Text(title.to_string()));
        doc.add(1, Value::Text(body.to_string()));
        doc.add(2, Value::Keyword(tag.to_string()));
        writer.add(&doc).expect("should index");
    }

    dir.append(writer.flush().expect("flushed"))
        .expect("should append");

    AppState {
        index: dir,
        analyzer: Analyzer::english(),
        started: Instant::now(),
    }
}

/// Builds a request without going through a socket.
fn request(method: &str, path: &str, query: &str, body: &str) -> Request {
    Request {
        method: method.to_string(),
        path: path.to_string(),
        query: parse_query(query),
        headers: HashMap::new(),
        body: body.to_string(),
    }
}

/// Parses a response body, failing loudly if it is not JSON.
fn json(response: &Response) -> quarry_core::Json {
    quarry_core::json::parse(&response.body)
        .unwrap_or_else(|err| panic!("body should be json: {err}\n{}", response.body))
}

// ---- percent decoding ---------------------------------------------------

#[test]
fn decodes_a_percent_escape() {
    assert_eq!(percent_decode("a%20b"), "a b");
}

#[test]
fn decodes_a_plus_as_a_space() {
    assert_eq!(percent_decode("a+b"), "a b");
}

#[test]
fn decodes_several_escapes() {
    assert_eq!(percent_decode("title%3Arust"), "title:rust");
}

#[test]
fn decodes_a_multibyte_escape() {
    assert_eq!(percent_decode("caf%C3%A9"), "café");
}

#[test]
fn leaves_plain_text_alone() {
    assert_eq!(percent_decode("plain"), "plain");
}

#[test]
fn leaves_a_malformed_escape_as_literal_text() {
    // A bare `%` is a user typo; searching for it beats returning a 400.
    assert_eq!(percent_decode("100%"), "100%");
    assert_eq!(percent_decode("%zz"), "%zz");
}

#[test]
fn handles_an_empty_string() {
    assert_eq!(percent_decode(""), "");
}

// ---- query strings ------------------------------------------------------

#[test]
fn parses_a_single_parameter() {
    let parsed = parse_query("q=rust");
    assert_eq!(parsed.get("q").map(String::as_str), Some("rust"));
}

#[test]
fn parses_several_parameters() {
    let parsed = parse_query("q=rust&limit=5");
    assert_eq!(parsed.len(), 2);
}

#[test]
fn parses_a_parameter_without_a_value() {
    let parsed = parse_query("highlight");
    assert_eq!(parsed.get("highlight").map(String::as_str), Some(""));
}

#[test]
fn decodes_parameter_values() {
    let parsed = parse_query("q=title%3Arust+ownership");
    assert_eq!(
        parsed.get("q").map(String::as_str),
        Some("title:rust ownership")
    );
}

#[test]
fn parses_an_empty_query_string() {
    assert!(parse_query("").is_empty());
}

#[test]
fn ignores_an_empty_pair() {
    assert_eq!(parse_query("a=1&&b=2").len(), 2);
}

// ---- request accessors --------------------------------------------------

#[test]
fn reads_a_parameter() {
    let req = request("GET", "/search", "q=rust", "");
    assert_eq!(req.param("q"), Some("rust"));
}

#[test]
fn falls_back_to_a_default_parameter() {
    let req = request("GET", "/search", "", "");
    assert_eq!(req.param_or("q", "fallback"), "fallback");
}

#[test]
fn parses_a_numeric_parameter() {
    let req = request("GET", "/search", "limit=25", "");
    assert_eq!(req.param_num("limit", 10), 25);
}

#[test]
fn a_non_numeric_parameter_uses_the_default() {
    let req = request("GET", "/search", "limit=lots", "");
    assert_eq!(req.param_num("limit", 10), 10);
}

#[test]
fn a_boolean_parameter_accepts_several_spellings() {
    // What someone poking at the API with curl will expect.
    for spelling in [
        "highlight",
        "highlight=1",
        "highlight=true",
        "highlight=yes",
    ] {
        let req = request("GET", "/search", spelling, "");
        assert!(req.param_flag("highlight"), "{spelling} should be truthy");
    }
}

#[test]
fn a_boolean_parameter_is_false_when_absent_or_denied() {
    assert!(!request("GET", "/search", "", "").param_flag("highlight"));
    assert!(!request("GET", "/search", "highlight=0", "").param_flag("highlight"));
    assert!(!request("GET", "/search", "highlight=false", "").param_flag("highlight"));
}

#[test]
fn keep_alive_is_the_http_11_default() {
    assert!(request("GET", "/health", "", "").wants_keep_alive());
}

#[test]
fn an_explicit_close_header_ends_the_connection() {
    let mut req = request("GET", "/health", "", "");
    req.headers
        .insert("connection".to_string(), "close".to_string());

    assert!(!req.wants_keep_alive());
}

#[test]
fn the_close_header_is_matched_case_insensitively() {
    let mut req = request("GET", "/health", "", "");
    req.headers
        .insert("connection".to_string(), "Close".to_string());

    assert!(!req.wants_keep_alive());
}

// ---- responses ----------------------------------------------------------

#[test]
fn renders_a_status_line_and_headers() {
    let rendered = Response::text(200, "hello").render(true);
    let text = String::from_utf8_lossy(&rendered);

    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text.contains("Content-Length: 5\r\n"));
    assert!(text.contains("Connection: keep-alive\r\n"));
}

#[test]
fn renders_connection_close_when_asked() {
    let rendered = Response::text(200, "x").render(false);
    assert!(String::from_utf8_lossy(&rendered).contains("Connection: close"));
}

#[test]
fn content_length_counts_bytes_not_characters() {
    // A multibyte body with a character count would truncate on the wire.
    let rendered = Response::text(200, "café").render(true);
    assert!(String::from_utf8_lossy(&rendered).contains("Content-Length: 5\r\n"));
}

#[test]
fn an_extra_header_is_rendered() {
    let response = Response::text(200, "x").with_header("X-Thing", "value");
    assert!(String::from_utf8_lossy(&response.render(true)).contains("X-Thing: value\r\n"));
}

#[test]
fn a_json_response_sets_its_content_type() {
    assert!(Response::json(200, "{}")
        .content_type
        .contains("application/json"));
}

#[test]
fn an_error_response_is_a_json_object() {
    let response = Response::error(404, "missing");
    assert_eq!(response.body, r#"{"error":"missing"}"#);
}

#[test]
fn an_error_message_with_quotes_is_escaped() {
    let response = Response::error(400, r#"bad "input""#);
    assert!(quarry_core::json::parse(&response.body).is_ok());
}

#[test]
fn the_headers_and_body_are_separated_by_a_blank_line() {
    let rendered = Response::text(200, "body").render(true);
    let text = String::from_utf8_lossy(&rendered);

    assert!(text.contains("\r\n\r\nbody"));
}

#[test]
fn reason_phrases_cover_the_statuses_in_use() {
    assert_eq!(reason_phrase(200), "OK");
    assert_eq!(reason_phrase(400), "Bad Request");
    assert_eq!(reason_phrase(404), "Not Found");
    assert_eq!(reason_phrase(405), "Method Not Allowed");
    assert_eq!(reason_phrase(413), "Payload Too Large");
    assert_eq!(reason_phrase(422), "Unprocessable Entity");
    assert_eq!(reason_phrase(500), "Internal Server Error");
}

#[test]
fn an_unmapped_status_gets_a_placeholder() {
    assert_eq!(reason_phrase(599), "Unknown");
}

// ---- json escaping ------------------------------------------------------

#[test]
fn escapes_quotes_and_backslashes() {
    assert_eq!(escape_json_string(r#"a"b\c"#), r#"a\"b\\c"#);
}

#[test]
fn escapes_control_characters() {
    assert_eq!(escape_json_string("a\nb"), "a\\nb");
    assert_eq!(escape_json_string("\u{01}"), "\\u0001");
}

#[test]
fn leaves_ordinary_text_alone() {
    assert_eq!(escape_json_string("plain text"), "plain text");
}

// The assertions are constant by design: this is a guard rail on a tuning
// constant, so it fails the build if someone changes it past a sane range.
#[allow(clippy::assertions_on_constants)]
#[test]
fn the_body_limit_is_large_enough_to_be_useful_and_small_enough_to_bound_memory() {
    // The body is read into a single allocation sized from Content-Length, so
    // this number is the per-request memory ceiling. It has to clear a
    // realistic batch of documents without letting one header claim a
    // gigabyte.
    assert!(MAX_BODY >= 1024 * 1024, "too small for a real batch");
    assert!(
        MAX_BODY <= 64 * 1024 * 1024,
        "too large to allocate per request"
    );
}

// ---- routing ------------------------------------------------------------

#[test]
fn health_reports_ok() {
    let temp = TempDir::new("health");
    let response = route(&request("GET", "/health", "", ""), &state(&temp));

    assert_eq!(response.status, 200);
    assert_eq!(
        json(&response)
            .get("status")
            .and_then(quarry_core::Json::as_str),
        Some("ok")
    );
}

#[test]
fn health_reports_the_version() {
    let temp = TempDir::new("health-version");
    let response = route(&request("GET", "/health", "", ""), &state(&temp));

    assert_eq!(
        json(&response)
            .get("version")
            .and_then(quarry_core::Json::as_str),
        Some(quarry_core::VERSION)
    );
}

#[test]
fn stats_reports_the_document_count() {
    let temp = TempDir::new("stats");
    let response = route(&request("GET", "/stats", "", ""), &state(&temp));

    assert_eq!(response.status, 200);
    assert_eq!(
        json(&response)
            .get("documents")
            .and_then(quarry_core::Json::as_f64),
        Some(2.0)
    );
}

#[test]
fn schema_returns_the_field_definitions() {
    let temp = TempDir::new("schema");
    let response = route(&request("GET", "/schema", "", ""), &state(&temp));

    assert_eq!(response.status, 200);
    assert!(response.body.contains("title"));
}

#[test]
fn search_finds_a_document() {
    let temp = TempDir::new("search");
    let response = route(&request("GET", "/search", "q=ownership", ""), &state(&temp));

    assert_eq!(response.status, 200);
    assert_eq!(
        json(&response)
            .get("total")
            .and_then(quarry_core::Json::as_f64),
        Some(1.0)
    );
}

#[test]
fn search_returns_the_stored_document() {
    let temp = TempDir::new("search-doc");
    let response = route(&request("GET", "/search", "q=ownership", ""), &state(&temp));

    assert!(response.body.contains("Rust ownership"));
}

#[test]
fn search_echoes_the_query_back() {
    let temp = TempDir::new("search-echo");
    let response = route(&request("GET", "/search", "q=ownership", ""), &state(&temp));

    assert_eq!(
        json(&response)
            .get("query")
            .and_then(quarry_core::Json::as_str),
        Some("ownership")
    );
}

#[test]
fn search_reports_how_long_it_took() {
    let temp = TempDir::new("search-timing");
    let response = route(&request("GET", "/search", "q=ownership", ""), &state(&temp));

    assert!(json(&response).get("took_us").is_some());
}

#[test]
fn search_without_a_query_is_a_422() {
    let temp = TempDir::new("search-empty");
    let response = route(&request("GET", "/search", "", ""), &state(&temp));

    assert_eq!(response.status, 422);
}

#[test]
fn a_malformed_query_is_a_400() {
    // The client's mistake, with the parser's message naming the position.
    let temp = TempDir::new("search-bad");
    let response = route(
        &request("GET", "/search", "q=title%3A%28unclosed", ""),
        &state(&temp),
    );

    assert_eq!(response.status, 400);
}

#[test]
fn an_unknown_field_is_a_422_with_a_suggestion() {
    let temp = TempDir::new("search-field");
    let response = route(
        &request("GET", "/search", "q=titel%3Arust", ""),
        &state(&temp),
    );

    assert_eq!(response.status, 422);
    assert!(
        response.body.contains("did you mean"),
        "body was {}",
        response.body
    );
}

#[test]
fn the_limit_is_honoured() {
    let temp = TempDir::new("search-limit");
    let response = route(
        &request("GET", "/search", "q=rust+OR+python&limit=1", ""),
        &state(&temp),
    );

    let hits = json(&response)
        .get("hits")
        .and_then(quarry_core::Json::as_array)
        .map(<[quarry_core::Json]>::len);

    assert_eq!(hits, Some(1));
}

#[test]
fn the_limit_is_clamped_to_a_maximum() {
    // `?limit=100000000` would otherwise ask the collector for a heap that
    // size before a single document is scored.
    let temp = TempDir::new("search-clamp");
    let response = route(
        &request("GET", "/search", "q=rust&limit=999999999", ""),
        &state(&temp),
    );

    let hits = json(&response)
        .get("hits")
        .and_then(quarry_core::Json::as_array)
        .map_or(0, <[quarry_core::Json]>::len);

    assert!(hits <= MAX_LIMIT);
}

#[test]
fn highlights_are_returned_when_asked() {
    let temp = TempDir::new("search-highlight");
    let response = route(
        &request("GET", "/search", "q=ownership&highlight", ""),
        &state(&temp),
    );

    assert!(
        response.body.contains("<mark>"),
        "body was {}",
        response.body
    );
}

#[test]
fn highlights_are_omitted_by_default() {
    let temp = TempDir::new("search-nohighlight");
    let response = route(&request("GET", "/search", "q=ownership", ""), &state(&temp));

    assert!(!response.body.contains("highlights"));
}

#[test]
fn planner_notes_are_surfaced_to_the_caller() {
    // Without them a dropped stopword makes the result set look arbitrary.
    let temp = TempDir::new("search-notes");
    let response = route(
        &request("GET", "/search", "q=the+AND+ownership", ""),
        &state(&temp),
    );

    assert!(
        response.body.contains("notes"),
        "body was {}",
        response.body
    );
}

#[test]
fn post_search_reads_the_query_from_the_body() {
    let temp = TempDir::new("post");
    let response = route(
        &request("POST", "/search", "", r#"{"q":"ownership"}"#),
        &state(&temp),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        json(&response)
            .get("total")
            .and_then(quarry_core::Json::as_f64),
        Some(1.0)
    );
}

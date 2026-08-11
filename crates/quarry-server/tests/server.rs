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

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

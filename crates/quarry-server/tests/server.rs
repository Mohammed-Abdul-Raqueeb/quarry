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

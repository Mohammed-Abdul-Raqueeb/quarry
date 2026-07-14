//! The on-disk index directory.
//!
//! These touch the filesystem, so each builds its own directory under the
//! system temp dir and removes it afterwards. The names include the process id
//! and a counter so two tests running in parallel cannot collide.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::dir::{schema_from_json, schema_to_json, SCHEMA_FILE};
use quarry_core::json::parse;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::{Document, IndexDir, IndexWriter, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A directory that deletes itself when the test ends, including on panic.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quarry-test-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
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
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("schema should build")
}

fn segment_with(titles: &[&str]) -> quarry_core::Segment {
    let mut writer = IndexWriter::new(schema());
    for title in titles {
        let mut doc = Document::new();
        doc.add(0, Value::Text((*title).to_string()));
        writer.add(&doc).expect("should index");
    }
    writer.flush().expect("a document was added")
}

// ---- create and open ----------------------------------------------------

#[test]
fn creates_a_directory_and_writes_a_schema() {
    let temp = TempDir::new("create");
    let dir = IndexDir::create(&temp.path, &schema()).expect("should create");

    assert!(dir.path().join(SCHEMA_FILE).exists());
    assert_eq!(dir.doc_count(), 0);
}

#[test]
fn refuses_to_create_over_an_existing_index() {
    // Replacing the schema would orphan every segment written against it.
    let temp = TempDir::new("no-clobber");
    IndexDir::create(&temp.path, &schema()).expect("first create");

    assert!(IndexDir::create(&temp.path, &schema()).is_err());
}

#[test]
fn opens_an_index_that_was_created_earlier() {
    let temp = TempDir::new("reopen");
    IndexDir::create(&temp.path, &schema()).expect("should create");

    let reopened = IndexDir::open(&temp.path).expect("should open");
    assert_eq!(reopened.schema().len(), 2);
}

#[test]
fn opening_a_missing_directory_is_an_error() {
    let temp = TempDir::new("missing");
    assert!(IndexDir::open(temp.join("nope")).is_err());
}

#[test]
fn opening_a_directory_without_a_schema_says_so() {
    let temp = TempDir::new("not-an-index");
    std::fs::create_dir_all(&temp.path).expect("should create dir");

    let err = IndexDir::open(&temp.path).unwrap_err().to_string();
    assert!(err.contains("not a Quarry index"), "message was: {err}");
}

#[test]
fn open_or_create_creates_when_absent() {
    let temp = TempDir::new("open-or-create");
    let dir = IndexDir::open_or_create(&temp.path, &schema()).expect("should create");
    assert_eq!(dir.doc_count(), 0);
}

#[test]
fn open_or_create_opens_when_present() {
    let temp = TempDir::new("open-or-open");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    dir.append(segment_with(&["one"])).expect("should append");

    let reopened = IndexDir::open_or_create(&temp.path, &schema()).expect("should open");
    assert_eq!(reopened.doc_count(), 1);
}

// ---- appending ----------------------------------------------------------

#[test]
fn appends_a_segment_and_keeps_it_loaded() {
    let temp = TempDir::new("append");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");

    dir.append(segment_with(&["alpha"])).expect("should append");

    assert_eq!(dir.segments().len(), 1);
    assert_eq!(dir.doc_count(), 1);
}

#[test]
fn appended_segments_survive_a_reopen() {
    let temp = TempDir::new("persist");
    {
        let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
        dir.append(segment_with(&["alpha", "beta"]))
            .expect("should append");
    }

    let reopened = IndexDir::open(&temp.path).expect("should open");
    assert_eq!(reopened.doc_count(), 2);
    assert_eq!(reopened.segments()[0].index.doc_freq(0, "alpha"), 1);
}

#[test]
fn appending_numbers_segments_sequentially() {
    let temp = TempDir::new("numbering");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");

    let first = dir.append(segment_with(&["a"])).expect("should append");
    let second = dir.append(segment_with(&["b"])).expect("should append");

    assert!(first.to_string_lossy().contains("00000001"));
    assert!(second.to_string_lossy().contains("00000002"));
}

#[test]
fn segment_names_are_zero_padded_so_they_sort_numerically() {
    // Without padding, `10.qseg` would sort before `9.qseg` and a reopened
    // index would stitch segments together in the wrong document order.
    let temp = TempDir::new("padding");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");

    for i in 0..11 {
        dir.append(segment_with(&[&format!("doc{i}")]))
            .expect("should append");
    }

    let reopened = IndexDir::open(&temp.path).expect("should open");
    assert_eq!(reopened.segments().len(), 11);
}

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

#[test]
fn reopening_preserves_document_order_across_segments() {
    let temp = TempDir::new("order");
    {
        let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
        dir.append(segment_with(&["first"])).expect("should append");
        dir.append(segment_with(&["second"]))
            .expect("should append");
    }

    let reopened = IndexDir::open(&temp.path).expect("should open");
    assert_eq!(reopened.segments()[0].index.doc_freq(0, "first"), 1);
    assert_eq!(reopened.segments()[1].index.doc_freq(0, "second"), 1);
}

#[test]
fn appending_a_segment_from_another_schema_is_rejected() {
    let temp = TempDir::new("schema-guard");
    let other = Schema::builder()
        .text("different")
        .and_then(SchemaBuilder::build)
        .expect("should build");

    let mut dir = IndexDir::create(&temp.path, &other).expect("should create");
    assert!(dir.append(segment_with(&["x"])).is_err());
}

#[test]
fn an_appended_segment_leaves_no_temporary_file() {
    // The write is a temp file plus a rename; a leftover means the rename
    // did not happen.
    let temp = TempDir::new("no-temp");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    dir.append(segment_with(&["x"])).expect("should append");

    let leftovers = std::fs::read_dir(&temp.path)
        .expect("should list")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "tmp"))
        .count();

    assert_eq!(leftovers, 0);
}

// ---- merging ------------------------------------------------------------

#[test]
fn merging_fewer_than_two_segments_does_nothing() {
    let temp = TempDir::new("merge-one");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    dir.append(segment_with(&["only"])).expect("should append");

    assert!(dir.merge_all().expect("should merge").is_none());
    assert_eq!(dir.segments().len(), 1);
}

#[test]
fn merging_collapses_the_segments_on_disk() {
    let temp = TempDir::new("merge-many");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    for title in ["a", "b", "c"] {
        dir.append(segment_with(&[title])).expect("should append");
    }

    assert!(dir.merge_all().expect("should merge").is_some());
    assert_eq!(dir.segments().len(), 1);
    assert_eq!(dir.doc_count(), 3);
}

#[test]
fn a_merged_index_survives_a_reopen() {
    let temp = TempDir::new("merge-persist");
    {
        let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
        for title in ["alpha", "beta", "gamma"] {
            dir.append(segment_with(&[title])).expect("should append");
        }
        dir.merge_all().expect("should merge");
    }

    let reopened = IndexDir::open(&temp.path).expect("should open");
    assert_eq!(reopened.segments().len(), 1);
    assert_eq!(reopened.doc_count(), 3);
    assert_eq!(reopened.segments()[0].index.doc_freq(0, "gamma"), 1);
}

#[test]
fn merging_removes_the_old_segment_files() {
    let temp = TempDir::new("merge-cleanup");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    for title in ["a", "b", "c"] {
        dir.append(segment_with(&[title])).expect("should append");
    }
    dir.merge_all().expect("should merge");

    let files = std::fs::read_dir(&temp.path)
        .expect("should list")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "qseg"))
        .count();

    assert_eq!(files, 1);
}

// ---- housekeeping -------------------------------------------------------

#[test]
fn clean_temp_files_removes_an_interrupted_write() {
    let temp = TempDir::new("clean");
    let dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    std::fs::write(temp.join("00000009.qseg.tmp"), b"partial").expect("should write");

    assert_eq!(dir.clean_temp_files().expect("should clean"), 1);
}

#[test]
fn clean_temp_files_leaves_real_segments_alone() {
    let temp = TempDir::new("clean-safe");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    dir.append(segment_with(&["keep"])).expect("should append");

    dir.clean_temp_files().expect("should clean");

    assert_eq!(
        IndexDir::open(&temp.path).expect("should open").doc_count(),
        1
    );
}

#[test]
fn a_stray_temp_file_does_not_break_opening() {
    // The reader only considers `.qseg`, so leftovers are inert.
    let temp = TempDir::new("stray");
    {
        let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
        dir.append(segment_with(&["x"])).expect("should append");
    }
    std::fs::write(temp.join("00000099.qseg.tmp"), b"junk").expect("should write");

    assert_eq!(
        IndexDir::open(&temp.path).expect("should open").doc_count(),
        1
    );
}

#[test]
fn a_corrupt_segment_file_fails_the_open_and_names_itself() {
    let temp = TempDir::new("corrupt");
    {
        let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
        dir.append(segment_with(&["x"])).expect("should append");
    }
    std::fs::write(temp.join("00000001.qseg"), b"not a segment at all").expect("should write");

    let err = IndexDir::open(&temp.path).unwrap_err().to_string();
    assert!(err.contains("00000001"), "message was: {err}");
}

#[test]
fn size_on_disk_grows_with_appended_segments() {
    let temp = TempDir::new("size");
    let mut dir = IndexDir::create(&temp.path, &schema()).expect("should create");
    assert_eq!(dir.size_on_disk().expect("should size"), 0);

    dir.append(segment_with(&["some text here"]))
        .expect("should append");
    assert!(dir.size_on_disk().expect("should size") > 0);
}

// ---- schema serialization -----------------------------------------------

#[test]
fn a_schema_round_trips_through_json() {
    let original = schema();
    let restored = schema_from_json(&schema_to_json(&original)).expect("should parse");

    assert_eq!(restored, original);
}

#[test]
fn schema_json_records_every_option() {
    let original = Schema::builder()
        .text_boosted("title", 2.5)
        .and_then(SchemaBuilder::build)
        .expect("should build");

    let restored = schema_from_json(&schema_to_json(&original)).expect("should parse");
    let field = restored.field("title").expect("field exists");

    assert_eq!(field.options.boost, 2.5);
    assert!(field.options.positions);
}

#[test]
fn schema_json_preserves_field_order() {
    // Field ids are positional, so order is part of the contract.
    let restored = schema_from_json(&schema_to_json(&schema())).expect("should parse");
    assert_eq!(
        restored.field_by_id(0).map(|f| f.name.as_str()),
        Some("title")
    );
}

#[test]
fn parsing_a_schema_without_fields_is_an_error() {
    let json = parse(r#"{"version":1}"#).expect("valid json");
    assert!(schema_from_json(&json).is_err());
}

#[test]
fn parsing_a_schema_with_an_unknown_type_is_an_error() {
    let json = parse(r#"{"fields":[{"name":"x","type":"vector"}]}"#).expect("valid json");
    assert!(schema_from_json(&json).is_err());
}

#[test]
fn parsing_a_schema_field_without_a_name_is_an_error() {
    let json = parse(r#"{"fields":[{"type":"text"}]}"#).expect("valid json");
    assert!(schema_from_json(&json).is_err());
}

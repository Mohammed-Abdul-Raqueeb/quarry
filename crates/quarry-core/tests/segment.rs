//! Segment serialization, corruption detection and merging.
//!
//! A segment is the durability boundary, so the rejection cases matter as much
//! as the round trips. Each corruption below is a failure that actually happens
//! in the field: a process killed mid-write, a damaged byte, a schema changed
//! underneath an existing index.

use quarry_core::index::merge::{merge, merge_with_report, MergePolicy};
use quarry_core::index::segment::{schema_digest, shared_prefix_len, FORMAT_VERSION, MAGIC_END};
use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::{Document, IndexWriter, Value};

fn schema() -> Schema {
    Schema::builder()
        .text("title")
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("schema should build")
}

fn writer() -> IndexWriter {
    IndexWriter::new(schema())
}

/// A segment holding the given titles, one document each.
fn segment_with(titles: &[&str]) -> Segment {
    let mut writer = writer();
    for title in titles {
        let mut doc = Document::new();
        doc.add(0, Value::Text((*title).to_string()));
        writer.add(&doc).expect("should index");
    }
    writer.flush().expect("a document was added")
}

// ---- round trips --------------------------------------------------------

#[test]
fn a_segment_round_trips() {
    let original = segment_with(&["rust ownership", "python typing"]);
    let bytes = original.write();
    let decoded = Segment::read(&bytes).expect("should read");

    assert_eq!(decoded.doc_count(), 2);
    assert_eq!(decoded.meta.term_count, original.meta.term_count);
}

#[test]
fn a_round_tripped_segment_keeps_its_postings() {
    let original = segment_with(&["rust ownership"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    assert_eq!(decoded.index.doc_freq(0, "rust"), 1);
    assert_eq!(decoded.index.doc_freq(0, "ownership"), 1);
}

#[test]
fn a_round_tripped_segment_keeps_positions() {
    let original = segment_with(&["alpha beta gamma"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    let list = decoded.index.postings(0, "beta").expect("term exists");
    assert_eq!(list.get(0).map(|p| p.positions.clone()), Some(vec![1]));
}

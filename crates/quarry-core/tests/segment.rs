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

#[test]
fn a_round_tripped_segment_keeps_stored_fields() {
    let original = segment_with(&["rust ownership"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    let doc = decoded.store.document(0).expect("document stored");
    assert_eq!(
        doc.value(0).map(Value::as_text),
        Some("rust ownership".to_string())
    );
}

#[test]
fn a_round_tripped_segment_keeps_field_statistics() {
    let original = segment_with(&["one two three", "four"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    assert_eq!(
        decoded.index.field_stats(0).total_length,
        original.index.field_stats(0).total_length
    );
}

#[test]
fn a_round_tripped_segment_keeps_per_document_lengths() {
    let original = segment_with(&["one two three", "four"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    assert_eq!(decoded.index.field_length(0, 0), 3);
    assert_eq!(decoded.index.field_length(1, 0), 1);
}

#[test]
fn a_segment_with_many_documents_round_trips() {
    let titles: Vec<String> = (0..200).map(|i| format!("document number {i}")).collect();
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();

    let original = segment_with(&refs);
    let decoded = Segment::read(&original.write()).expect("should read");

    assert_eq!(decoded.doc_count(), 200);
}

#[test]
fn a_segment_with_multibyte_terms_round_trips() {
    let original = segment_with(&["café naïve", "日本語 テスト"]);
    let decoded = Segment::read(&original.write()).expect("should read");

    assert_eq!(decoded.doc_count(), 2);
}

#[test]
fn serialization_is_deterministic() {
    // Two segments built the same way must serialize identically, or the
    // front-coded dictionary and the checksums would be unstable.
    let first = segment_with(&["rust ownership", "python typing"]);
    let second = segment_with(&["rust ownership", "python typing"]);

    assert_eq!(first.write(), second.write());
}

#[test]
fn a_segment_records_the_current_format_version() {
    assert_eq!(segment_with(&["x"]).meta.version, FORMAT_VERSION);
}

#[test]
fn a_segment_records_its_document_and_term_counts() {
    let segment = segment_with(&["alpha beta"]);
    assert_eq!(segment.meta.doc_count, 1);
    assert!(segment.meta.term_count >= 2);
}

#[test]
fn a_segment_with_one_document_is_not_empty() {
    assert!(!segment_with(&["x"]).is_empty());
}

// ---- framing and corruption --------------------------------------------

#[test]
fn rejects_a_buffer_that_is_too_short() {
    assert!(Segment::read(&[1, 2, 3]).is_err());
}

#[test]
fn rejects_an_empty_buffer() {
    assert!(Segment::read(&[]).is_err());
}

#[test]
fn rejects_a_missing_leading_magic() {
    let mut bytes = segment_with(&["x"]).write();
    bytes[0] = b'X';

    let err = Segment::read(&bytes).unwrap_err().to_string();
    assert!(err.contains("magic"), "message was: {err}");
}

#[test]
fn rejects_a_truncated_segment() {
    // The failure a process killed mid-write leaves behind.
    let bytes = segment_with(&["rust ownership", "more text here"]).write();
    let cut = &bytes[..bytes.len() - 20];

    assert!(Segment::read(cut).is_err());
}

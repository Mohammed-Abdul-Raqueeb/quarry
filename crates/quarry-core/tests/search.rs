//! Query execution, phrase matching and snippets, end to end.

use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::search::highlight::{find_spans, prefix_snippet, snippet, HighlightOptions};
use quarry_core::search::searcher::{has_ordered_run, intersect_sorted, union_sorted};
use quarry_core::{
    Analyzer, BooleanQuery, Document, IndexWriter, PhraseQuery, Searcher, TermQuery, Value,
};
use std::sync::OnceLock;

/// The shared test schema.
///
/// Returned by reference from a `OnceLock` because `Searcher` borrows it for
/// as long as it lives, and a fresh temporary per call would be dropped while
/// still borrowed.
fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Schema::builder()
            .text("title")
            .and_then(|b| b.text("body"))
            .and_then(|b| b.keyword("tag"))
            .and_then(SchemaBuilder::build)
            .expect("schema should build")
    })
}

/// Builds a one-segment index from `(title, body, tag)` triples.
fn index(docs: &[(&str, &str, &str)]) -> Vec<Segment> {
    let mut writer = IndexWriter::new(schema().clone());

    for (title, body, tag) in docs {
        let mut doc = Document::new();
        doc.add(0, Value::Text((*title).to_string()));
        doc.add(1, Value::Text((*body).to_string()));
        doc.add(2, Value::Keyword((*tag).to_string()));
        writer.add(&doc).expect("should index");
    }

    writer.flush().map(|s| vec![s]).unwrap_or_default()
}

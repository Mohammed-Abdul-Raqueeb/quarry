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

fn corpus() -> Vec<Segment> {
    index(&[
        (
            "Rust ownership",
            "The borrow checker enforces ownership",
            "rust",
        ),
        (
            "Zero cost abstractions",
            "Iterators compile to tight loops",
            "rust",
        ),
        (
            "Fearless concurrency",
            "Rust prevents data races at compile time",
            "rust",
        ),
        (
            "Python typing",
            "Gradual typing annotates incrementally",
            "python",
        ),
        (
            "Borrowing in practice",
            "Most complaints are real aliasing bugs",
            "rust",
        ),
    ])
}

/// The analyzed form of a word, as the index stored it.
fn term(word: &str) -> String {
    Analyzer::english()
        .terms(word)
        .into_iter()
        .next()
        .unwrap_or_else(|| word.to_string())
}

fn titles(searcher: &Searcher<'_>, results: &quarry_core::Results) -> Vec<String> {
    results
        .hits
        .iter()
        .filter_map(|hit| searcher.document(hit))
        .filter_map(|doc| doc.value(0).map(Value::as_text))
        .collect()
}

// ---- term queries -------------------------------------------------------

#[test]
fn finds_a_document_by_a_title_term() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(0, term("ownership")));

    assert_eq!(
        titles(&searcher, &searcher.search(&query, 10).expect("search")),
        ["Rust ownership"]
    );
}

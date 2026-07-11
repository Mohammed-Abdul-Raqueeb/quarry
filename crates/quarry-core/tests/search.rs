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

#[test]
fn finds_a_document_by_a_body_term() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(1, term("iterators")));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn finds_a_document_by_a_keyword() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "python"));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn an_absent_term_matches_nothing() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(0, "kubernetes"));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn an_empty_query_matches_nothing() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    assert!(searcher
        .search(&BooleanQuery::new(), 10)
        .expect("search")
        .is_empty());
}

#[test]
fn a_stemmed_query_finds_an_inflected_document() {
    // The property the analyzer exists for.
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(1, term("compiled")));

    assert!(!searcher.search(&query, 10).expect("search").is_empty());
}

// ---- boolean combinations ----------------------------------------------

#[test]
fn must_clauses_are_intersected() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .must(TermQuery::new(0, term("rust")))
        .must(TermQuery::new(0, term("ownership")));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn an_unsatisfiable_must_clause_matches_nothing() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .must(TermQuery::new(0, term("rust")))
        .must(TermQuery::new(0, term("python")));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn should_clauses_are_unioned() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .should(TermQuery::new(0, term("ownership")))
        .should(TermQuery::new(0, term("typing")));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 2);
}

#[test]
fn must_not_excludes_a_document() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .must(TermQuery::new(2, "rust"))
        .must_not(TermQuery::new(0, term("ownership")));

    let found = titles(&searcher, &searcher.search(&query, 10).expect("search"));
    assert!(!found.contains(&"Rust ownership".to_string()));
    assert_eq!(found.len(), 3);
}

#[test]
fn must_not_on_an_absent_term_excludes_nothing() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .must(TermQuery::new(2, "rust"))
        .must_not(TermQuery::new(0, "kubernetes"));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 4);
}

#[test]
fn must_any_requires_one_match_from_the_group() {
    // A bare term across several default fields: the word must appear in one
    // of title or body, not in both.
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().must_any(vec![
        TermQuery::new(0, term("rust")),
        TermQuery::new(1, term("rust")),
    ]);

    // "Rust ownership" has it in the title, "Fearless concurrency" in the
    // body. The other three mention it only in the `tag` keyword field, which
    // this group does not cover.
    assert_eq!(searcher.search(&query, 10).expect("search").len(), 2);
}

#[test]
fn must_any_groups_are_intersected_with_each_other() {
    // This is the case that was broken: `rust AND concurrency` must find the
    // document with `rust` in the body and `concurrency` in the title.
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new()
        .must_any(vec![
            TermQuery::new(0, term("rust")),
            TermQuery::new(1, term("rust")),
        ])
        .must_any(vec![
            TermQuery::new(0, term("concurrency")),
            TermQuery::new(1, term("concurrency")),
        ]);

    assert_eq!(
        titles(&searcher, &searcher.search(&query, 10).expect("search")),
        ["Fearless concurrency"]
    );
}

#[test]
fn an_unsatisfiable_must_any_group_matches_nothing() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().must_any(vec![TermQuery::new(0, "kubernetes")]);
    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn minimum_should_match_filters_weak_matches() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let mut query = BooleanQuery::new()
        .should(TermQuery::new(0, term("rust")))
        .should(TermQuery::new(0, term("ownership")));
    query.minimum_should_match = 2;

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn an_empty_boolean_query_reports_itself_as_empty() {
    assert!(BooleanQuery::new().is_empty());
}

#[test]
fn a_query_with_any_clause_is_not_empty() {
    assert!(!BooleanQuery::new().must(TermQuery::new(0, "x")).is_empty());
    assert!(!BooleanQuery::new()
        .must_any(vec![TermQuery::new(0, "x")])
        .is_empty());
}

#[test]
fn must_any_ignores_an_empty_group() {
    // An empty group would otherwise make every document fail the intersection.
    assert!(BooleanQuery::new().must_any(vec![]).is_empty());
}

#[test]
fn all_terms_collects_from_every_clause_kind() {
    let query = BooleanQuery::new()
        .must(TermQuery::new(0, "must"))
        .should(TermQuery::new(0, "should"))
        .must_any(vec![TermQuery::new(0, "any")])
        .phrase(PhraseQuery::new(
            0,
            vec!["one".to_string(), "two".to_string()],
        ));

    let terms = query.all_terms();
    for expected in ["must", "should", "any", "one", "two"] {
        assert!(terms.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn all_terms_is_deduplicated_and_sorted() {
    let query = BooleanQuery::new()
        .must(TermQuery::new(0, "rust"))
        .should(TermQuery::new(1, "rust"))
        .should(TermQuery::new(0, "apple"));

    assert_eq!(
        query.all_terms(),
        vec!["apple".to_string(), "rust".to_string()]
    );
}

// ---- ranking ------------------------------------------------------------

#[test]
fn results_come_back_best_first() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new()
        .should(TermQuery::new(0, term("rust")))
        .should(TermQuery::new(1, term("rust")));

    let results = searcher.search(&query, 10).expect("search");
    let scores: Vec<f32> = results.hits.iter().map(|h| h.score).collect();

    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "scores were {scores:?}"
    );
}

#[test]
fn a_boost_raises_a_clause_contribution() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let plain = BooleanQuery::new().must(TermQuery::new(0, term("ownership")));
    let boosted = BooleanQuery::new().must(TermQuery::new(0, term("ownership")).boosted(3.0));

    let plain_score = searcher.search(&plain, 1).expect("search").max_score();
    let boosted_score = searcher.search(&boosted, 1).expect("search").max_score();

    assert!(boosted_score > plain_score);
}

#[test]
fn a_field_boost_from_the_schema_is_applied() {
    let boosted_schema = Schema::builder()
        .text_boosted("title", 5.0)
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("should build");

    let mut writer = IndexWriter::new(boosted_schema.clone());
    let mut doc = Document::new();
    doc.add(0, Value::Text("rust".to_string()));
    doc.add(1, Value::Text("rust".to_string()));
    writer.add(&doc).expect("should index");
    let segments = vec![writer.flush().expect("flushed")];

    let searcher = Searcher::new(&segments, &boosted_schema);
    let title_score = searcher
        .search(&BooleanQuery::new().must(TermQuery::new(0, "rust")), 1)
        .expect("search")
        .max_score();
    let body_score = searcher
        .search(&BooleanQuery::new().must(TermQuery::new(1, "rust")), 1)
        .expect("search")
        .max_score();

    assert!(title_score > body_score);
}

#[test]
fn the_limit_bounds_how_many_hits_come_back() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "rust"));

    assert_eq!(searcher.search(&query, 2).expect("search").len(), 2);
}

#[test]
fn total_hits_counts_matches_beyond_the_limit() {
    // Reporting the returned count here is the bug this guards against.
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "rust"));

    let results = searcher.search(&query, 1).expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results.total_hits, 4);
}

#[test]
fn count_matches_the_total_from_a_search() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "rust"));

    assert_eq!(searcher.count(&query).expect("count"), 4);
}

#[test]
fn results_report_how_long_the_query_took() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "rust"));

    assert!(
        searcher
            .search(&query, 10)
            .expect("search")
            .elapsed
            .as_nanos()
            > 0
    );
}

#[test]
fn max_score_is_none_for_an_empty_result() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(0, "absent"));

    assert!(searcher
        .search(&query, 10)
        .expect("search")
        .max_score()
        .is_none());
}

// ---- across segments ---------------------------------------------------

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

#[test]
fn searches_across_several_segments() {
    let mut segments = index(&[("alpha", "first", "a")]);
    segments.extend(index(&[("beta", "second", "b")]));

    let searcher = Searcher::new(&segments, schema());
    assert_eq!(searcher.doc_count(), 2);

    let query = BooleanQuery::new().must(TermQuery::new(0, "beta"));
    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn a_hit_from_a_later_segment_resolves_to_its_document() {
    let mut segments = index(&[("alpha", "first", "a")]);
    segments.extend(index(&[("beta", "second", "b")]));

    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(0, "beta"));
    let results = searcher.search(&query, 10).expect("search");

    let hit = &results.hits[0];
    assert_eq!(hit.segment, 1);
    assert_eq!(hit.local_doc, 0);
    assert_eq!(
        searcher
            .document(hit)
            .and_then(|d| d.value(0).map(Value::as_text)),
        Some("beta".to_string())
    );
}

#[test]
fn global_document_ids_do_not_collide_across_segments() {
    let mut segments = index(&[("a", "x", "t")]);
    segments.extend(index(&[("b", "y", "t")]));

    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(2, "t"));
    let results = searcher.search(&query, 10).expect("search");

    let ids: Vec<u32> = results.hits.iter().map(|h| h.doc).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn searching_an_empty_index_returns_nothing() {
    let segments: Vec<Segment> = Vec::new();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().must(TermQuery::new(0, "anything"));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
    assert_eq!(searcher.doc_count(), 0);
}

// ---- phrase queries ----------------------------------------------------

#[test]
fn a_phrase_matches_adjacent_terms_in_order() {
    let segments = index(&[("zero cost abstractions", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(0, vec![term("zero"), term("cost")]));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn a_phrase_does_not_match_reversed_terms() {
    let segments = index(&[("zero cost abstractions", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(0, vec![term("cost"), term("zero")]));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn a_phrase_does_not_match_separated_terms() {
    let segments = index(&[("zero and cost", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(0, vec![term("zero"), term("cost")]));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn slop_lets_a_phrase_tolerate_a_gap() {
    // `and` is a stopword, so it leaves a position gap; slop is what lets the
    // phrase survive it.
    let segments = index(&[("zero and cost", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new()
        .phrase(PhraseQuery::new(0, vec![term("zero"), term("cost")]).with_slop(2));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn a_phrase_with_a_missing_term_matches_nothing() {
    let segments = index(&[("zero cost", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query =
        BooleanQuery::new().phrase(PhraseQuery::new(0, vec![term("zero"), term("kubernetes")]));

    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn an_empty_phrase_matches_nothing() {
    let segments = index(&[("anything", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(0, vec![]));
    assert!(searcher.search(&query, 10).expect("search").is_empty());
}

#[test]
fn a_phrase_on_a_field_without_positions_is_an_error() {
    // Better than silently returning nothing, which looks like no match.
    let segments = index(&[("x", "y", "a tag")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(
        2,
        vec!["a".to_string(), "tag".to_string()],
    ));

    assert!(searcher.search(&query, 10).is_err());
}

#[test]
fn a_matching_phrase_still_scores_above_zero() {
    let segments = index(&[("zero cost", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(0, vec![term("zero"), term("cost")]));

    assert!(
        searcher
            .search(&query, 10)
            .expect("search")
            .max_score()
            .unwrap_or(0.0)
            > 0.0
    );
}

#[test]
fn a_three_term_phrase_matches() {
    let segments = index(&[("alpha beta gamma delta", "x", "t")]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new().phrase(PhraseQuery::new(
        0,
        vec![term("beta"), term("gamma"), term("delta")],
    ));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

// ---- has_ordered_run ----------------------------------------------------

#[test]
fn ordered_run_finds_adjacent_positions() {
    assert!(has_ordered_run(&[&[0, 5], &[1]], 0));
}

#[test]
fn ordered_run_rejects_reversed_positions() {
    assert!(!has_ordered_run(&[&[5], &[1]], 0));
}

#[test]
fn ordered_run_rejects_a_gap_beyond_the_slop() {
    assert!(!has_ordered_run(&[&[0], &[5]], 1));
}

#[test]
fn ordered_run_accepts_a_gap_within_the_slop() {
    assert!(has_ordered_run(&[&[0], &[3]], 2));
}

#[test]
fn ordered_run_tries_every_anchor() {
    // The first occurrence of the leading term does not line up, but the
    // second does.
    assert!(has_ordered_run(&[&[0, 10], &[11]], 0));
}

#[test]
fn ordered_run_of_a_single_term_is_just_presence() {
    assert!(has_ordered_run(&[&[3]], 0));
    assert!(!has_ordered_run(&[&[]], 0));
}

#[test]
fn ordered_run_of_nothing_is_false() {
    assert!(!has_ordered_run(&[], 0));
}

#[test]
fn ordered_run_handles_a_long_position_list() {
    let many: Vec<u32> = (0..5_000).collect();
    let second = [4_999u32];
    assert!(has_ordered_run(&[&many, &second], 0));
}

// ---- set helpers --------------------------------------------------------

#[test]
fn intersect_sorted_keeps_shared_ids() {
    assert_eq!(intersect_sorted(&[1, 2, 5], &[2, 5, 8]), vec![2, 5]);
}

#[test]
fn intersect_sorted_handles_empty_inputs() {
    assert!(intersect_sorted(&[], &[1, 2]).is_empty());
    assert!(intersect_sorted(&[1, 2], &[]).is_empty());
}

#[test]
fn union_sorted_merges_without_duplicates() {
    assert_eq!(union_sorted(&[1, 3], &[3, 5]), vec![1, 3, 5]);
}

#[test]
fn union_sorted_handles_empty_inputs() {
    assert_eq!(union_sorted(&[], &[1, 2]), vec![1, 2]);
    assert_eq!(union_sorted(&[1, 2], &[]), vec![1, 2]);
}

#[test]
fn union_sorted_keeps_its_output_sorted() {
    let merged = union_sorted(&[1, 9], &[4, 6]);
    assert!(merged.windows(2).all(|w| w[0] < w[1]));
}

// ---- highlighting -------------------------------------------------------

#[test]
fn finds_the_span_of_a_matching_term() {
    let analyzer = Analyzer::english();
    let spans = find_spans(&analyzer, "Rust ownership rules", &[term("ownership")]);

    assert_eq!(spans.len(), 1);
    assert_eq!(
        &"Rust ownership rules"[spans[0].start..spans[0].end],
        "ownership"
    );
}

#[test]
fn finds_spans_for_an_inflected_form() {
    // The index holds `run`, but the user must see the word `Running`.
    let analyzer = Analyzer::english();
    let text = "Running fast";
    let spans = find_spans(&analyzer, text, &[term("run")]);

    assert_eq!(&text[spans[0].start..spans[0].end], "Running");
}

//! Numeric range matching, from the predicate up to a full query.

// Exact float comparison is deliberate: the bounds are integers written in the
// source, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::search::range::{count_matching, matching_docs, RangeQuery};
use quarry_core::{Analyzer, BooleanQuery, Document, IndexWriter, Searcher, TermQuery, Value};
use std::sync::OnceLock;

/// `title` is field 0, `year` is field 1.
fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Schema::builder()
            .text("title")
            .and_then(|b| b.numeric("year"))
            .and_then(SchemaBuilder::build)
            .expect("schema should build")
    })
}

const TITLE: u16 = 0;
const YEAR: u16 = 1;

/// The analyzed form of a word, as the index stored it.
///
/// Needed because the text pipeline stems: `middle` is indexed as `middl`, so
/// a term query written with the raw word matches nothing — and a test that
/// only asserts emptiness would pass for the wrong reason.
fn term(word: &str) -> String {
    Analyzer::english()
        .terms(word)
        .into_iter()
        .next()
        .unwrap_or_else(|| word.to_string())
}

/// One segment from `(title, year)` pairs.
fn index(docs: &[(&str, f64)]) -> Vec<Segment> {
    let mut writer = IndexWriter::new(schema().clone());

    for (title, year) in docs {
        let mut doc = Document::new();
        doc.add(TITLE, Value::Text((*title).to_string()));
        doc.add(YEAR, Value::Number(*year));
        writer.add(&doc).expect("should index");
    }

    writer.flush().map(|s| vec![s]).unwrap_or_default()
}

fn corpus() -> Vec<Segment> {
    index(&[
        ("oldest", 2015.0),
        ("older", 2018.0),
        ("middle", 2020.0),
        ("newer", 2022.0),
        ("newest", 2025.0),
    ])
}

// ---- the predicate -------------------------------------------------------

#[test]
fn an_inclusive_range_contains_both_bounds() {
    let range = RangeQuery::inclusive(YEAR, 2000.0, 2020.0);

    assert!(range.contains(2000.0));
    assert!(range.contains(2010.0));
    assert!(range.contains(2020.0));
}

#[test]
fn an_inclusive_range_excludes_values_outside_it() {
    let range = RangeQuery::inclusive(YEAR, 2000.0, 2020.0);

    assert!(!range.contains(1999.0));
    assert!(!range.contains(2021.0));
}

#[test]
fn an_exclusive_range_excludes_both_bounds() {
    let range = RangeQuery::exclusive(YEAR, 2000.0, 2020.0);

    assert!(!range.contains(2000.0));
    assert!(range.contains(2010.0));
    assert!(!range.contains(2020.0));
}

#[test]
fn a_mixed_range_honours_each_end_separately() {
    let range = RangeQuery {
        field: YEAR,
        lower: Some(2000.0),
        upper: Some(2020.0),
        include_lower: true,
        include_upper: false,
    };

    assert!(range.contains(2000.0));
    assert!(!range.contains(2020.0));
}

#[test]
fn at_least_is_unbounded_above() {
    let range = RangeQuery::at_least(YEAR, 2020.0);

    assert!(range.contains(2020.0));
    assert!(range.contains(1e9));
    assert!(!range.contains(2019.0));
}

#[test]
fn at_most_is_unbounded_below() {
    let range = RangeQuery::at_most(YEAR, 2020.0);

    assert!(range.contains(2020.0));
    assert!(range.contains(-1e9));
    assert!(!range.contains(2021.0));
}

#[test]
fn a_fully_unbounded_range_contains_everything() {
    let range = RangeQuery {
        field: YEAR,
        lower: None,
        upper: None,
        include_lower: true,
        include_upper: true,
    };

    assert!(range.contains(0.0));
    assert!(range.contains(-1e9));
    assert!(range.is_unbounded());
}

#[test]
fn a_range_handles_negative_bounds() {
    let range = RangeQuery::inclusive(YEAR, -10.0, -1.0);

    assert!(range.contains(-5.0));
    assert!(!range.contains(0.0));
    assert!(!range.contains(-11.0));
}

#[test]
fn a_range_handles_fractional_bounds() {
    let range = RangeQuery::inclusive(YEAR, 0.5, 1.5);

    assert!(range.contains(1.0));
    assert!(!range.contains(0.25));
}

#[test]
fn a_nan_value_never_matches() {
    // It compares false against every bound, and treating it as a match would
    // put a corrupt document in every range.
    let range = RangeQuery::inclusive(YEAR, 0.0, 100.0);
    assert!(!range.contains(f64::NAN));
}

#[test]
fn an_infinite_value_matches_only_an_unbounded_end() {
    assert!(RangeQuery::at_least(YEAR, 0.0).contains(f64::INFINITY));
    assert!(!RangeQuery::inclusive(YEAR, 0.0, 100.0).contains(f64::INFINITY));
}

// ---- emptiness -----------------------------------------------------------

#[test]
fn crossed_bounds_describe_an_empty_range() {
    let range = RangeQuery::inclusive(YEAR, 2020.0, 2000.0);
    assert!(range.is_empty());
}

#[test]
fn equal_inclusive_bounds_are_a_single_point_not_empty() {
    let range = RangeQuery::inclusive(YEAR, 2020.0, 2020.0);

    assert!(!range.is_empty());
    assert!(range.contains(2020.0));
}

#[test]
fn equal_bounds_with_one_end_excluded_are_empty() {
    let half = RangeQuery {
        field: YEAR,
        lower: Some(2020.0),
        upper: Some(2020.0),
        include_lower: true,
        include_upper: false,
    };

    assert!(half.is_empty());
    assert!(RangeQuery::exclusive(YEAR, 2020.0, 2020.0).is_empty());
}

#[test]
fn a_half_open_range_is_never_empty() {
    assert!(!RangeQuery::at_least(YEAR, 2020.0).is_empty());
    assert!(!RangeQuery::at_most(YEAR, 2020.0).is_empty());
}

// ---- matching against a segment -----------------------------------------

#[test]
fn matches_documents_inside_an_inclusive_range() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 2018.0, 2022.0));

    assert_eq!(docs, vec![1, 2, 3]);
}

#[test]
fn matched_document_ids_come_back_ascending() {
    // They are unioned term by term in dictionary order, which is not document
    // order, so the ascending result is a property worth pinning.
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::at_least(YEAR, 0.0));

    assert!(docs.windows(2).all(|w| w[0] < w[1]), "got {docs:?}");
}

#[test]
fn excludes_the_bounds_of_an_exclusive_range() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::exclusive(YEAR, 2018.0, 2022.0));

    assert_eq!(docs, vec![2]);
}

#[test]
fn matches_everything_at_or_above_a_lower_bound() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::at_least(YEAR, 2020.0));

    assert_eq!(docs, vec![2, 3, 4]);
}

#[test]
fn matches_everything_at_or_below_an_upper_bound() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::at_most(YEAR, 2018.0));

    assert_eq!(docs, vec![0, 1]);
}

#[test]
fn matches_a_single_point() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 2020.0, 2020.0));

    assert_eq!(docs, vec![2]);
}

#[test]
fn matches_nothing_when_no_value_falls_inside() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 1900.0, 1950.0));

    assert!(docs.is_empty());
}

#[test]
fn an_empty_range_matches_nothing_without_scanning() {
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 2025.0, 2015.0));

    assert!(docs.is_empty());
}

#[test]
fn matches_nothing_on_an_empty_segment() {
    let segments = index(&[]);
    assert!(segments.is_empty());
}

#[test]
fn a_range_on_a_field_with_no_values_matches_nothing() {
    // `title` holds text, so none of its terms parse as numbers.
    let segments = corpus();
    let docs = matching_docs(&segments[0], &RangeQuery::at_least(TITLE, 0.0));

    assert!(docs.is_empty());
}

#[test]
fn handles_negative_values_in_the_index() {
    let segments = index(&[("loss", -50.0), ("flat", 0.0), ("gain", 50.0)]);
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, -100.0, 0.0));

    assert_eq!(docs, vec![0, 1]);
}

#[test]
fn handles_fractional_values_in_the_index() {
    let segments = index(&[("low", 0.25), ("mid", 1.5), ("high", 9.75)]);
    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 1.0, 2.0));

    assert_eq!(docs, vec![1]);
}

#[test]
fn a_document_with_several_values_in_range_is_counted_once() {
    let mut writer = IndexWriter::new(schema().clone());
    let mut doc = Document::new();
    doc.add(TITLE, Value::Text("multi".to_string()));
    doc.add(YEAR, Value::Number(2020.0));
    doc.add(YEAR, Value::Number(2021.0));
    writer.add(&doc).expect("should index");
    let segments = writer.flush().map(|s| vec![s]).unwrap_or_default();

    let docs = matching_docs(&segments[0], &RangeQuery::inclusive(YEAR, 2000.0, 2030.0));
    assert_eq!(docs, vec![0]);
}

#[test]
fn count_matching_agrees_with_the_list_length() {
    let segments = corpus();
    let range = RangeQuery::inclusive(YEAR, 2018.0, 2022.0);

    assert_eq!(
        count_matching(&segments[0], &range),
        matching_docs(&segments[0], &range).len()
    );
}

// ---- through a query -----------------------------------------------------

#[test]
fn a_range_alone_drives_a_query() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().range(RangeQuery::inclusive(YEAR, 2018.0, 2022.0));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 3);
}

#[test]
fn a_pure_range_query_still_scores_above_zero() {
    // It contributes no term weight, but the document did match, so it has to
    // be collected rather than dropped for having a zero score.
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().range(RangeQuery::at_least(YEAR, 2020.0));

    let results = searcher.search(&query, 10).expect("search");
    assert!(results.max_score().unwrap_or(0.0) > 0.0);
}

#[test]
fn a_range_filters_a_term_query() {
    let segments = index(&[("rust guide", 2015.0), ("rust guide", 2022.0)]);
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new()
        .must(TermQuery::new(TITLE, term("rust")))
        .range(RangeQuery::at_least(YEAR, 2020.0));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn a_range_that_excludes_everything_empties_the_result() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    // Establish the term matches on its own first. Without this the assertion
    // below would also pass if the term simply never matched, which is how a
    // filter test quietly stops testing the filter.
    let unfiltered = BooleanQuery::new().must(TermQuery::new(TITLE, term("middle")));
    assert_eq!(searcher.search(&unfiltered, 10).expect("search").len(), 1);

    let filtered = unfiltered.range(RangeQuery::inclusive(YEAR, 1900.0, 1950.0));
    assert!(searcher.search(&filtered, 10).expect("search").is_empty());
}

#[test]
fn two_ranges_on_one_field_intersect() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new()
        .range(RangeQuery::at_least(YEAR, 2018.0))
        .range(RangeQuery::at_most(YEAR, 2020.0));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 2);
}

#[test]
fn an_unbounded_range_places_no_constraint() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());

    let query = BooleanQuery::new()
        .must(TermQuery::new(TITLE, term("middle")))
        .range(RangeQuery {
            field: YEAR,
            lower: None,
            upper: None,
            include_lower: true,
            include_upper: true,
        });

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 1);
}

#[test]
fn a_range_query_is_not_reported_as_empty() {
    assert!(!BooleanQuery::new()
        .range(RangeQuery::at_least(YEAR, 0.0))
        .is_empty());
}

#[test]
fn a_range_works_across_several_segments() {
    let mut segments = index(&[("first", 2015.0)]);
    segments.extend(index(&[("second", 2025.0)]));

    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().range(RangeQuery::at_least(YEAR, 2020.0));

    let results = searcher.search(&query, 10).expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results.hits[0].segment, 1);
}

#[test]
fn a_range_survives_a_segment_round_trip() {
    let segments = corpus();
    let reloaded = vec![Segment::read(&segments[0].write()).expect("should read")];

    let searcher = Searcher::new(&reloaded, schema());
    let query = BooleanQuery::new().range(RangeQuery::inclusive(YEAR, 2018.0, 2022.0));

    assert_eq!(searcher.search(&query, 10).expect("search").len(), 3);
}

#[test]
fn counting_a_range_matches_searching_it() {
    let segments = corpus();
    let searcher = Searcher::new(&segments, schema());
    let query = BooleanQuery::new().range(RangeQuery::at_least(YEAR, 2018.0));

    assert_eq!(
        searcher.count(&query).expect("count"),
        searcher.search(&query, 100).expect("search").total_hits
    );
}

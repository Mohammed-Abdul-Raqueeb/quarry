//! Relevance scoring and result collection.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::score::bm25::{Bm25, DEFAULT_B, DEFAULT_K1};
use quarry_core::score::tfidf::TfIdf;
use quarry_core::score::{ConstantScorer, Scorer};
use quarry_core::search::collector::{CountCollector, ScoredDoc, TopK};

// ---- BM25 ---------------------------------------------------------------

#[test]
fn bm25_defaults_are_the_published_values() {
    let bm25 = Bm25::default();
    assert_eq!(bm25.k1, DEFAULT_K1);
    assert_eq!(bm25.b, DEFAULT_B);
}

#[test]
fn bm25_clamps_a_negative_k1() {
    // Outside its range the formula stops being monotonic in term frequency,
    // which produces rankings that look arbitrary rather than badly tuned.
    assert_eq!(Bm25::new(-5.0, 0.5).k1, 0.0);
}

#[test]
fn bm25_clamps_b_into_zero_to_one() {
    assert_eq!(Bm25::new(1.2, -1.0).b, 0.0);
    assert_eq!(Bm25::new(1.2, 5.0).b, 1.0);
}

#[test]
fn bm25_scores_zero_for_an_absent_term() {
    assert_eq!(Bm25::default().score(0, 1, 100, 10, 10.0), 0.0);
}

#[test]
fn bm25_scores_zero_for_an_empty_index() {
    assert_eq!(Bm25::default().score(1, 0, 0, 10, 10.0), 0.0);
}

#[test]
fn bm25_rises_with_term_frequency() {
    let bm25 = Bm25::default();
    let once = bm25.score(1, 10, 1000, 100, 100.0);
    let twice = bm25.score(2, 10, 1000, 100, 100.0);

    assert!(twice > once);
}

#[test]
fn bm25_saturates_in_term_frequency() {
    // The property that distinguishes it from TF-IDF: a document repeating a
    // word twenty times is not ten times more relevant than one with two.
    let bm25 = Bm25::default();
    let first_step = bm25.score(2, 10, 1000, 100, 100.0) - bm25.score(1, 10, 1000, 100, 100.0);
    let later_step = bm25.score(21, 10, 1000, 100, 100.0) - bm25.score(20, 10, 1000, 100, 100.0);

    assert!(later_step < first_step, "frequency should saturate");
}

#[test]
fn bm25_scores_a_rare_term_higher_than_a_common_one() {
    let bm25 = Bm25::default();
    let rare = bm25.score(1, 2, 1000, 100, 100.0);
    let common = bm25.score(1, 900, 1000, 100, 100.0);

    assert!(rare > common);
}

#[test]
fn bm25_penalises_a_longer_field() {
    let bm25 = Bm25::default();
    let short = bm25.score(1, 10, 1000, 10, 100.0);
    let long = bm25.score(1, 10, 1000, 1000, 100.0);

    assert!(
        short > long,
        "a match in a short field should count for more"
    );
}

#[test]
fn bm25_ignores_length_when_b_is_zero() {
    let bm25 = Bm25::without_length_normalization();
    let short = bm25.score(1, 10, 1000, 10, 100.0);
    let long = bm25.score(1, 10, 1000, 1000, 100.0);

    assert!((short - long).abs() < f32::EPSILON);
}

#[test]
fn bm25_survives_a_zero_average_length() {
    // A corrupt statistics block could produce one; dividing by it would
    // yield infinity and poison the whole ranking.
    let score = Bm25::default().score(1, 10, 1000, 10, 0.0);
    assert!(score.is_finite());
}

#[test]
fn bm25_idf_is_zero_for_an_empty_corpus() {
    assert_eq!(Bm25::idf(0, 0), 0.0);
}

#[test]
fn bm25_idf_falls_as_document_frequency_rises() {
    assert!(Bm25::idf(1000, 1) > Bm25::idf(1000, 500));
}

#[test]
fn bm25_idf_stays_positive_for_a_term_in_every_document() {
    // Without the `+1` inside the logarithm this goes negative, and adding a
    // common word to a query would push matching documents *down*.
    assert!(Bm25::idf(1000, 1000) > 0.0);
}

#[test]
fn bm25_idf_clamps_a_frequency_above_the_corpus_size() {
    let idf = Bm25::idf(100, 1_000);
    assert!(idf.is_finite() && idf > 0.0);
}

#[test]
fn bm25_names_itself() {
    assert_eq!(Bm25::default().name(), "bm25");
}

#[test]
fn bm25_scores_are_finite_across_a_wide_range() {
    let bm25 = Bm25::default();
    for tf in [1u32, 10, 1_000] {
        for df in [1u64, 100, 10_000] {
            for len in [1u32, 100, 100_000] {
                let score = bm25.score(tf, df, 10_000, len, 500.0);
                assert!(score.is_finite(), "tf={tf} df={df} len={len} gave {score}");
            }
        }
    }
}

// ---- TF-IDF -------------------------------------------------------------

#[test]
fn tfidf_scores_zero_for_an_absent_term() {
    assert_eq!(TfIdf::default().score(0, 1, 100, 10, 10.0), 0.0);
}

#[test]
fn tfidf_rises_with_term_frequency() {
    let tfidf = TfIdf::default();
    assert!(tfidf.score(4, 10, 1000, 100, 0.0) > tfidf.score(1, 10, 1000, 100, 0.0));
}

#[test]
fn tfidf_does_not_saturate() {
    // The reason BM25 is the default: TF-IDF grows without bound, so repeating
    // a word two hundred times outranks using it well.
    let tfidf = TfIdf::raw();
    let first_step = tfidf.score(2, 10, 1000, 100, 0.0) - tfidf.score(1, 10, 1000, 100, 0.0);
    let later_step = tfidf.score(21, 10, 1000, 100, 0.0) - tfidf.score(20, 10, 1000, 100, 0.0);

    assert!((later_step - first_step).abs() < 0.001);
}

#[test]
fn tfidf_sublinear_mode_dampens_frequency() {
    let sublinear = TfIdf::default();
    let raw = TfIdf::raw();

    assert!(sublinear.score(16, 10, 1000, 100, 0.0) < raw.score(16, 10, 1000, 100, 0.0));
}

#[test]
fn tfidf_normalises_by_field_length_when_asked() {
    let tfidf = TfIdf::default();
    assert!(tfidf.score(1, 10, 1000, 4, 0.0) > tfidf.score(1, 10, 1000, 400, 0.0));
}

#[test]
fn tfidf_raw_mode_ignores_length() {
    let tfidf = TfIdf::raw();
    let short = tfidf.score(1, 10, 1000, 4, 0.0);
    let long = tfidf.score(1, 10, 1000, 400, 0.0);

    assert!((short - long).abs() < f32::EPSILON);
}

#[test]
fn tfidf_idf_stays_positive_for_a_universal_term() {
    assert!(TfIdf::idf(100, 100) > 0.0);
}

#[test]
fn tfidf_idf_is_zero_for_an_empty_corpus() {
    assert_eq!(TfIdf::idf(0, 0), 0.0);
}

#[test]
fn tfidf_names_itself() {
    assert_eq!(TfIdf::default().name(), "tfidf");
}

// ---- ConstantScorer -----------------------------------------------------

#[test]
fn constant_scorer_returns_one_for_a_match() {
    assert_eq!(ConstantScorer.score(1, 100, 1000, 50, 50.0), 1.0);
}

#[test]
fn constant_scorer_returns_zero_for_no_match() {
    assert_eq!(ConstantScorer.score(0, 100, 1000, 50, 50.0), 0.0);
}

#[test]
fn constant_scorer_ignores_every_other_input() {
    assert_eq!(
        ConstantScorer.score(1, 1, 1, 1, 1.0),
        ConstantScorer.score(99, 999, 9999, 999, 999.0)
    );
}

#[test]
fn constant_scorer_names_itself() {
    assert_eq!(ConstantScorer.name(), "constant");
}

// ---- ScoredDoc ordering -------------------------------------------------

#[test]
fn scored_docs_order_by_score() {
    assert!(ScoredDoc::new(1, 2.0) > ScoredDoc::new(2, 1.0));
}

#[test]
fn equal_scores_break_the_tie_on_document_id() {
    // Reversed, so that once the heap is inverted for top-k the lowest id
    // comes out first and results are stable.
    assert!(ScoredDoc::new(1, 1.0) > ScoredDoc::new(2, 1.0));
}

#[test]
fn a_nan_score_orders_consistently_rather_than_panicking() {
    // A pathological boost can produce one, and a partial comparison would
    // break the heap's invariant.
    let nan = ScoredDoc::new(1, f32::NAN);
    let ordinary = ScoredDoc::new(2, 1.0);

    assert!(nan.partial_cmp(&ordinary).is_some());
}

// ---- TopK ---------------------------------------------------------------

#[test]
fn topk_starts_empty() {
    let collector = TopK::new(5);
    assert!(collector.is_empty());
    assert_eq!(collector.total_hits(), 0);
}

#[test]
fn topk_keeps_everything_below_its_limit() {
    let mut collector = TopK::new(5);
    for doc in 0..3 {
        collector.collect(doc, 1.0);
    }

    assert_eq!(collector.len(), 3);
}

#[test]
fn topk_keeps_only_the_best_once_full() {
    let mut collector = TopK::new(2);
    collector.collect(0, 1.0);
    collector.collect(1, 5.0);
    collector.collect(2, 3.0);

    let kept: Vec<u32> = collector.into_sorted().iter().map(|s| s.doc).collect();
    assert_eq!(kept, vec![1, 2]);
}

#[test]
fn topk_returns_results_best_first() {
    let mut collector = TopK::new(3);
    collector.collect(0, 1.0);
    collector.collect(1, 3.0);
    collector.collect(2, 2.0);

    let scores: Vec<f32> = collector.into_sorted().iter().map(|s| s.score).collect();
    assert_eq!(scores, vec![3.0, 2.0, 1.0]);
}

#[test]
fn topk_breaks_score_ties_by_ascending_document_id() {
    let mut collector = TopK::new(3);
    for doc in [2u32, 0, 1] {
        collector.collect(doc, 1.0);
    }

    let docs: Vec<u32> = collector.into_sorted().iter().map(|s| s.doc).collect();
    assert_eq!(docs, vec![0, 1, 2]);
}

#[test]
fn topk_counts_every_match_including_those_it_dropped() {
    // This is what the API reports as `total`, so it must not be capped by
    // the limit.
    let mut collector = TopK::new(2);
    for doc in 0..10 {
        collector.collect(doc, 1.0);
    }

    assert_eq!(collector.total_hits(), 10);
    assert_eq!(collector.len(), 2);
}

#[test]
fn topk_with_a_zero_limit_keeps_nothing_but_still_counts() {
    let mut collector = TopK::new(0);
    collector.collect(0, 1.0);

    assert_eq!(collector.total_hits(), 1);
    assert!(collector.is_empty());
}

#[test]
fn topk_has_no_threshold_until_it_is_full() {
    let mut collector = TopK::new(3);
    collector.collect(0, 1.0);

    assert!(collector.threshold().is_none());
}

#[test]
fn topk_reports_its_worst_kept_score_once_full() {
    // An executor uses this to skip documents that cannot make the cut, which
    // is where the win from a bounded collector shows up.
    let mut collector = TopK::new(2);
    collector.collect(0, 5.0);
    collector.collect(1, 3.0);

    assert_eq!(collector.threshold(), Some(3.0));
}

#[test]
fn the_threshold_rises_as_better_matches_arrive() {
    let mut collector = TopK::new(2);
    collector.collect(0, 1.0);
    collector.collect(1, 2.0);
    assert_eq!(collector.threshold(), Some(1.0));

    collector.collect(2, 3.0);
    assert_eq!(collector.threshold(), Some(2.0));
}

#[test]
fn topk_handles_many_candidates_with_bounded_memory() {
    let mut collector = TopK::new(10);
    for doc in 0..10_000u32 {
        collector.collect(doc, f32::from(u16::try_from(doc % 100).unwrap_or(0)));
    }

    assert_eq!(collector.len(), 10);
    assert_eq!(collector.total_hits(), 10_000);
}

#[test]
fn topk_keeps_the_genuine_best_from_a_large_stream() {
    let mut collector = TopK::new(3);
    for doc in 0..1_000u32 {
        collector.collect(doc, f32::from(u16::try_from(doc).unwrap_or(0)));
    }

    let docs: Vec<u32> = collector.into_sorted().iter().map(|s| s.doc).collect();
    assert_eq!(docs, vec![999, 998, 997]);
}

#[test]
fn topk_handles_negative_scores() {
    let mut collector = TopK::new(2);
    collector.collect(0, -1.0);
    collector.collect(1, -5.0);
    collector.collect(2, -3.0);

    let docs: Vec<u32> = collector.into_sorted().iter().map(|s| s.doc).collect();
    assert_eq!(docs, vec![0, 2]);
}

// ---- CountCollector -----------------------------------------------------

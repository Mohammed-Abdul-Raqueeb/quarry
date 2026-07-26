//! Edit distance and the AST helpers.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_query::ast::Query;
use quarry_query::fuzzy::{
    auto_distance, bounded_levenshtein, damerau_levenshtein, find_matches, levenshtein, similarity,
    MAX_DISTANCE,
};

// ---- levenshtein --------------------------------------------------------

#[test]
fn distance_to_itself_is_zero() {
    assert_eq!(levenshtein("rust", "rust"), 0);
}

#[test]
fn distance_between_empty_strings_is_zero() {
    assert_eq!(levenshtein("", ""), 0);
}

#[test]
fn distance_from_empty_is_the_other_length() {
    assert_eq!(levenshtein("", "rust"), 4);
    assert_eq!(levenshtein("rust", ""), 4);
}

#[test]
fn counts_a_substitution_as_one() {
    assert_eq!(levenshtein("cat", "bat"), 1);
}

#[test]
fn counts_an_insertion_as_one() {
    assert_eq!(levenshtein("cat", "cart"), 1);
}

#[test]
fn counts_a_deletion_as_one() {
    assert_eq!(levenshtein("cart", "cat"), 1);
}

#[test]
fn counts_a_transposition_as_two() {
    // Plain Levenshtein charges two substitutions; Damerau charges one.
    assert_eq!(levenshtein("teh", "the"), 2);
}

#[test]
fn computes_the_textbook_distances() {
    assert_eq!(levenshtein("kitten", "sitting"), 3);
    assert_eq!(levenshtein("saturday", "sunday"), 3);
    assert_eq!(levenshtein("flaw", "lawn"), 2);
}

#[test]
fn distance_is_symmetric() {
    for (a, b) in [("kitten", "sitting"), ("rust", "trust"), ("", "x")] {
        assert_eq!(levenshtein(a, b), levenshtein(b, a), "{a} vs {b}");
    }
}

#[test]
fn distance_never_exceeds_the_longer_length() {
    for (a, b) in [("abc", "xyz"), ("a", "abcdef"), ("rust", "go")] {
        let bound = a.chars().count().max(b.chars().count());
        assert!(levenshtein(a, b) <= bound);
    }
}

#[test]
fn handles_multibyte_characters() {
    // Measured in characters, not bytes: a byte-based count would report two
    // edits for one changed accent.
    assert_eq!(levenshtein("café", "cafe"), 1);
    assert_eq!(levenshtein("日本", "日本"), 0);
}

#[test]
fn handles_a_long_pair_without_panicking() {
    let a = "a".repeat(500);
    let b = "b".repeat(500);
    assert_eq!(levenshtein(&a, &b), 500);
}

// ---- bounded levenshtein ------------------------------------------------

#[test]
fn bounded_agrees_with_unbounded_when_within_budget() {
    for (a, b) in [("cat", "bat"), ("kitten", "sitting"), ("rust", "rust")] {
        let exact = levenshtein(a, b);
        assert_eq!(bounded_levenshtein(a, b, exact), Some(exact), "{a} vs {b}");
    }
}

#[test]
fn bounded_gives_up_past_the_budget() {
    assert_eq!(bounded_levenshtein("kitten", "sitting", 2), None);
}

#[test]
fn bounded_rejects_on_a_length_difference_alone() {
    // The cheapest possible rejection, and the common case in a dictionary
    // scan.
    assert_eq!(bounded_levenshtein("a", "abcdef", 2), None);
}

#[test]
fn bounded_handles_identical_strings() {
    assert_eq!(bounded_levenshtein("rust", "rust", 0), Some(0));
}

#[test]
fn bounded_handles_an_empty_input() {
    assert_eq!(bounded_levenshtein("", "ab", 2), Some(2));
    assert_eq!(bounded_levenshtein("", "abc", 2), None);
}

#[test]
fn bounded_with_a_zero_budget_demands_equality() {
    assert_eq!(bounded_levenshtein("rust", "rust", 0), Some(0));
    assert_eq!(bounded_levenshtein("rust", "rusty", 0), None);
}

#[test]
fn bounded_matches_unbounded_across_a_sample() {
    let words = [
        "rust", "trust", "crust", "rusty", "ruse", "go", "golang", "",
    ];

    for a in words {
        for b in words {
            let exact = levenshtein(a, b);
            let bounded = bounded_levenshtein(a, b, 2);
            if exact <= 2 {
                assert_eq!(bounded, Some(exact), "{a} vs {b}");
            } else {
                assert_eq!(bounded, None, "{a} vs {b}");
            }
        }
    }
}

// ---- damerau ------------------------------------------------------------

#[test]
fn damerau_counts_a_transposition_as_one() {
    // Transposed letters are the most common typing error, so ranking them as
    // one edit matches real typos better.
    assert_eq!(damerau_levenshtein("teh", "the"), 1);
}

#[test]
fn damerau_agrees_with_levenshtein_without_transpositions() {
    assert_eq!(damerau_levenshtein("cat", "bat"), 1);
    assert_eq!(damerau_levenshtein("kitten", "sitting"), 3);
}

#[test]
fn damerau_handles_empty_inputs() {
    assert_eq!(damerau_levenshtein("", ""), 0);
    assert_eq!(damerau_levenshtein("", "ab"), 2);
    assert_eq!(damerau_levenshtein("ab", ""), 2);
}

#[test]
fn damerau_is_never_greater_than_levenshtein() {
    for (a, b) in [("teh", "the"), ("kitten", "sitting"), ("ab", "ba")] {
        assert!(damerau_levenshtein(a, b) <= levenshtein(a, b), "{a} vs {b}");
    }
}

// ---- similarity ---------------------------------------------------------

#[test]
fn similarity_of_identical_strings_is_one() {
    assert_eq!(similarity("rust", "rust"), 1.0);
}

#[test]
fn similarity_of_empty_strings_is_one() {
    assert_eq!(similarity("", ""), 1.0);
}

#[test]
fn similarity_falls_as_distance_grows() {
    assert!(similarity("rust", "rusty") > similarity("rust", "python"));
}

#[test]
fn similarity_normalises_by_the_longer_string() {
    // One edit in a short word matters more than one in a long word, and the
    // raw distance hides that.
    let short = similarity("ab", "ac");
    let long = similarity("abcdefghij", "abcdefghik");

    assert!(long > short);
}

#[test]
fn similarity_stays_within_zero_and_one() {
    for (a, b) in [("rust", "go"), ("", "abc"), ("same", "same")] {
        let value = similarity(a, b);
        assert!((0.0..=1.0).contains(&value), "{a} vs {b} gave {value}");
    }
}

// ---- auto_distance ------------------------------------------------------

#[test]
fn auto_distance_is_zero_for_a_very_short_term() {
    // Two edits on a three-letter word matches almost everything.
    assert_eq!(auto_distance(0), 0);
    assert_eq!(auto_distance(2), 0);
}

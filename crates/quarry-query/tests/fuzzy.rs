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

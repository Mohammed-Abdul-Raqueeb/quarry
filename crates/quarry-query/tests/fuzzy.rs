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

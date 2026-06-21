//! Porter stemmer behaviour.
//!
//! The vocabulary cases come from Porter's own published test set. They are
//! grouped by the rule each one exercises, because when a change breaks one
//! group it is almost always that step's condition that is wrong, and a flat
//! list of two hundred words tells you nothing about which.

use quarry_core::tokenize::stemmer::stem;

/// Asserts a whole group at once, reporting every failure rather than the first.
fn check(cases: &[(&str, &str)]) {
    let mut failures = Vec::new();

    for (input, expected) in cases {
        let actual = stem(input);
        if actual != *expected {
            failures.push(format!("{input} -> {actual} (expected {expected})"));
        }
    }

    assert!(
        failures.is_empty(),
        "stem mismatches:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn leaves_short_words_alone() {
    // Every rule would leave a two-letter word empty or meaningless.
    check(&[
        ("a", "a"),
        ("be", "be"),
        ("to", "to"),
        ("it", "it"),
        ("ox", "ox"),
    ]);
}

#[test]
fn strips_simple_plurals() {
    check(&[
        ("cats", "cat"),
        ("dogs", "dog"),
        ("tables", "tabl"),
        ("words", "word"),
        ("plays", "plai"),
    ]);
}

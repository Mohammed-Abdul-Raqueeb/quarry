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

#[test]
fn collapses_sses_to_ss() {
    check(&[
        ("caresses", "caress"),
        ("classes", "class"),
        ("glasses", "glass"),
    ]);
}

#[test]
fn turns_ies_into_i() {
    check(&[("ponies", "poni"), ("ties", "ti"), ("cries", "cri")]);
}

#[test]
fn keeps_a_terminal_double_s() {
    // `ss` is not a plural marker, so it survives step 1a untouched.
    check(&[("caress", "caress"), ("class", "class"), ("grass", "grass")]);
}

#[test]
fn removes_ed_when_the_stem_has_a_vowel() {
    check(&[
        ("plastered", "plaster"),
        ("rolled", "roll"),
        ("walked", "walk"),
    ]);
}

#[test]
fn keeps_ed_when_the_stem_has_no_vowel() {
    // `bl` has no vowel, so removing `-ed` would leave nothing meaningful.
    check(&[("bled", "bled"), ("fed", "fed"), ("sped", "sped")]);
}

#[test]
fn removes_ing_when_the_stem_has_a_vowel() {
    check(&[
        ("motoring", "motor"),
        ("meeting", "meet"),
        ("reading", "read"),
    ]);
}

#[test]
fn keeps_ing_when_the_stem_has_no_vowel() {
    check(&[("sing", "sing"), ("king", "king"), ("bring", "bring")]);
}

#[test]
fn restores_an_e_after_at_bl_and_iz() {
    // Step 1b's repair pass adds the `e` back, and step 5a keeps it here
    // because `siz` is a short CVC stem that needs it to stay distinct.
    assert_eq!(stem("sized"), "size");
}

#[test]
fn a_restored_e_is_dropped_again_when_the_stem_is_long() {
    // Step 1b turns `conflated` into `conflate`, then step 5a removes the `e`
    // because the stem has more than one vowel-consonant pair. The restored
    // `e` surviving is the exception, not the rule — easy to misread the
    // algorithm's worked examples as final output when they are intermediate.
    check(&[("conflated", "conflat"), ("troubled", "troubl")]);
}

#[test]
fn collapses_a_doubled_consonant_after_removing_a_suffix() {
    check(&[("hopping", "hop"), ("tanned", "tan"), ("falling", "fall")]);
}

#[test]
fn keeps_a_doubled_l_s_or_z() {
    // These three are exempted from the collapse rule by name.
    check(&[("falling", "fall"), ("hissing", "hiss"), ("fizzed", "fizz")]);
}

#[test]
fn restores_an_e_for_a_short_cvc_stem() {
    check(&[("filing", "file"), ("hoping", "hope")]);
}

#[test]
fn turns_a_terminal_y_into_i() {
    check(&[("happy", "happi"), ("sky", "sky"), ("try", "try")]);
}

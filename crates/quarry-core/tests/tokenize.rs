//! Tokenizers, filters and the analyzer pipeline.

use quarry_core::tokenize::ascii::{fold, needs_folding, AsciiFolding};
use quarry_core::tokenize::ngram::{EdgeNgramTokenizer, NgramTokenizer};
use quarry_core::tokenize::standard::{KeywordTokenizer, StandardTokenizer, WhitespaceTokenizer};
use quarry_core::tokenize::stopwords::{StopwordFilter, ENGLISH};
use quarry_core::tokenize::{Analyzer, LengthFilter, Lowercase, Token, TokenFilter, Tokenizer};

fn texts(tokens: &[Token]) -> Vec<&str> {
    tokens.iter().map(|t| t.text.as_str()).collect()
}

// ---- StandardTokenizer ----------------------------------------------------

#[test]
fn splits_a_sentence_on_spaces() {
    let tokens = StandardTokenizer::default().tokenize("the quick brown fox");
    assert_eq!(texts(&tokens), ["the", "quick", "brown", "fox"]);
}

#[test]
fn splits_on_punctuation() {
    let tokens = StandardTokenizer::default().tokenize("one,two;three!four");
    assert_eq!(texts(&tokens), ["one", "two", "three", "four"]);
}

#[test]
fn collapses_runs_of_separators() {
    let tokens = StandardTokenizer::default().tokenize("a   ,,,   b");
    assert_eq!(texts(&tokens), ["a", "b"]);
}

#[test]
fn returns_nothing_for_an_empty_input() {
    assert!(StandardTokenizer::default().tokenize("").is_empty());
}

#[test]
fn returns_nothing_for_only_separators() {
    assert!(StandardTokenizer::default().tokenize("  ,.;!  ").is_empty());
}

#[test]
fn keeps_an_internal_apostrophe() {
    // Splitting `wasn't` into `wasn` and `t` makes it unfindable.
    let tokens = StandardTokenizer::default().tokenize("it wasn't there");
    assert_eq!(texts(&tokens), ["it", "wasn't", "there"]);
}

#[test]
fn keeps_a_typographic_apostrophe() {
    let tokens = StandardTokenizer::default().tokenize("it wasn\u{2019}t there");
    assert_eq!(texts(&tokens), ["it", "wasn\u{2019}t", "there"]);
}

#[test]
fn keeps_an_internal_hyphen() {
    let tokens = StandardTokenizer::default().tokenize("a well-known e-mail");
    assert_eq!(texts(&tokens), ["a", "well-known", "e-mail"]);
}

#[test]
fn keeps_an_internal_period() {
    let tokens = StandardTokenizer::default().tokenize("version v1.2.3 shipped");
    assert_eq!(texts(&tokens), ["version", "v1.2.3", "shipped"]);
}

#[test]
fn strips_a_trailing_sentence_period() {
    // The period is punctuation here, not part of the word.
    let tokens = StandardTokenizer::default().tokenize("the end.");
    assert_eq!(texts(&tokens), ["the", "end"]);
}

#[test]
fn strips_a_leading_hyphen_from_a_list_item() {
    let tokens = StandardTokenizer::default().tokenize("- item");
    assert_eq!(texts(&tokens), ["item"]);
}

#[test]
fn strips_surrounding_quotes_but_keeps_the_word() {
    let tokens = StandardTokenizer::default().tokenize("he said 'hello'");
    assert_eq!(texts(&tokens), ["he", "said", "hello"]);
}

#[test]
fn drops_a_token_that_is_only_internal_characters() {
    let tokens = StandardTokenizer::default().tokenize("a --- b");
    assert_eq!(texts(&tokens), ["a", "b"]);
}

#[test]
fn keeps_digits_as_tokens() {
    let tokens = StandardTokenizer::default().tokenize("page 42 of 100");
    assert_eq!(texts(&tokens), ["page", "42", "of", "100"]);
}

#[test]
fn keeps_an_underscore_inside_a_token() {
    let tokens = StandardTokenizer::default().tokenize("snake_case_name");
    assert_eq!(texts(&tokens), ["snake_case_name"]);
}

#[test]
fn numbers_positions_from_zero() {
    let tokens = StandardTokenizer::default().tokenize("a b c");
    assert_eq!(
        tokens.iter().map(|t| t.position).collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn reports_byte_offsets_that_index_the_original_text() {
    let input = "the quick fox";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn reports_offsets_correctly_after_trimming_punctuation() {
    let input = "hello, 'world'.";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn reports_offsets_correctly_through_multibyte_text() {
    let input = "café naïve dog";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

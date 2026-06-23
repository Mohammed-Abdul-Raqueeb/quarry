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

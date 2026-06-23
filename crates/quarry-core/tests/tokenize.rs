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

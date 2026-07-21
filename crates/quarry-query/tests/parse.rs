//! Lexing and parsing the query language.

// Exact float comparison is deliberate here: the values are constants from
// the source or small sums chosen to be exactly representable, so a tolerance
// would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_query::lexer::{tokenize, Tok, MAX_QUERY_LEN};
use quarry_query::parser::MAX_DEPTH;
use quarry_query::{parse, Query, QueryError};

fn parsed(input: &str) -> Query {
    parse(input).unwrap_or_else(|err| panic!("{input:?} should parse, got {err}"))
}

fn rejected(input: &str) -> QueryError {
    parse(input).expect_err(&format!("{input:?} should have been rejected"))
}

// ---- lexer --------------------------------------------------------------

#[test]
fn lexes_a_bare_word() {
    assert_eq!(
        tokenize("rust").ok(),
        Some(vec![Tok::Word("rust".to_string())])
    );
}

#[test]
fn lexes_several_words() {
    assert_eq!(tokenize("a b").ok().map(|t| t.len()), Some(2));
}

#[test]
fn skips_surrounding_whitespace() {
    assert_eq!(tokenize("  rust  ").ok().map(|t| t.len()), Some(1));
}

#[test]
fn lexes_an_empty_query_to_no_tokens() {
    assert_eq!(tokenize("").ok().map(|t| t.len()), Some(0));
}

#[test]
fn lexes_the_word_operators() {
    assert_eq!(tokenize("AND").ok(), Some(vec![Tok::And]));
    assert_eq!(tokenize("OR").ok(), Some(vec![Tok::Or]));
    assert_eq!(tokenize("NOT").ok(), Some(vec![Tok::Not]));
    assert_eq!(tokenize("TO").ok(), Some(vec![Tok::To]));
}

#[test]
fn operators_are_case_sensitive() {
    // `and` is an English word someone will search for.
    assert_eq!(
        tokenize("and").ok(),
        Some(vec![Tok::Word("and".to_string())])
    );
}

#[test]
fn lexes_the_symbolic_operators() {
    assert_eq!(tokenize("&&").ok(), Some(vec![Tok::And]));
    assert_eq!(tokenize("||").ok(), Some(vec![Tok::Or]));
    assert_eq!(tokenize("!").ok(), Some(vec![Tok::Not]));
}

#[test]
fn rejects_a_single_ampersand_or_pipe() {
    assert!(tokenize("&").is_err());
    assert!(tokenize("|").is_err());
}

#[test]
fn lexes_the_punctuation_tokens() {
    assert_eq!(tokenize("(").ok(), Some(vec![Tok::LParen]));
    assert_eq!(tokenize(")").ok(), Some(vec![Tok::RParen]));
    assert_eq!(tokenize("[").ok(), Some(vec![Tok::LBracket]));
    assert_eq!(tokenize("]").ok(), Some(vec![Tok::RBracket]));
    assert_eq!(tokenize("{").ok(), Some(vec![Tok::LBrace]));
    assert_eq!(tokenize("}").ok(), Some(vec![Tok::RBrace]));
    assert_eq!(tokenize(":").ok(), Some(vec![Tok::Colon]));
    assert_eq!(tokenize("~").ok(), Some(vec![Tok::Tilde]));
    assert_eq!(tokenize("^").ok(), Some(vec![Tok::Caret]));
    assert_eq!(tokenize("*").ok(), Some(vec![Tok::Star]));
    assert_eq!(tokenize("+").ok(), Some(vec![Tok::Plus]));
}

#[test]
fn lexes_a_number() {
    assert_eq!(tokenize("42").ok(), Some(vec![Tok::Number(42.0)]));
}

#[test]
fn lexes_a_fractional_number() {
    assert_eq!(tokenize("1.5").ok(), Some(vec![Tok::Number(1.5)]));
}

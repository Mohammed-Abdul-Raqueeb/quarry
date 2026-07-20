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

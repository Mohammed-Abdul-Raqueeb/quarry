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

#[test]
fn a_word_with_digits_is_not_a_number() {
    assert_eq!(tokenize("3d").ok(), Some(vec![Tok::Word("3d".to_string())]));
}

#[test]
fn lexes_a_quoted_phrase() {
    assert_eq!(
        tokenize("\"zero cost\"").ok(),
        Some(vec![Tok::Quoted("zero cost".to_string())])
    );
}

#[test]
fn lexes_an_escaped_quote_inside_a_phrase() {
    assert_eq!(
        tokenize(r#""say \"hi\"""#).ok(),
        Some(vec![Tok::Quoted("say \"hi\"".to_string())])
    );
}

#[test]
fn rejects_an_unterminated_quote() {
    assert!(matches!(
        tokenize("\"open"),
        Err(QueryError::UnterminatedQuote { .. })
    ));
}

#[test]
fn a_leading_minus_is_negation() {
    assert_eq!(
        tokenize("-rust").ok(),
        Some(vec![Tok::Not, Tok::Word("rust".to_string())])
    );
}

#[test]
fn an_internal_minus_stays_part_of_the_word() {
    // `well-known` is one term, not a negation of `known`.
    assert_eq!(
        tokenize("well-known").ok(),
        Some(vec![Tok::Word("well-known".to_string())])
    );
}

#[test]
fn rejects_a_query_over_the_length_limit() {
    let long = "a".repeat(MAX_QUERY_LEN + 1);
    assert!(matches!(tokenize(&long), Err(QueryError::TooLong { .. })));
}

#[test]
fn accepts_a_query_at_the_length_limit() {
    let long = "a".repeat(MAX_QUERY_LEN);
    assert!(tokenize(&long).is_ok());
}

#[test]
fn starts_clause_identifies_clause_openers() {
    assert!(Tok::Word("x".to_string()).starts_clause());
    assert!(Tok::LParen.starts_clause());
    assert!(!Tok::And.starts_clause());
    assert!(!Tok::RParen.starts_clause());
}

// ---- parsing simple queries --------------------------------------------

#[test]
fn parses_a_bare_term() {
    assert_eq!(parsed("rust"), Query::term("rust"));
}

#[test]
fn parses_a_field_scoped_term() {
    assert_eq!(parsed("title:rust"), Query::field_term("title", "rust"));
}

#[test]
fn parses_a_number_as_a_term() {
    assert_eq!(parsed("2021"), Query::term("2021"));
}

#[test]
fn parses_a_field_scoped_number() {
    assert_eq!(parsed("year:2021"), Query::field_term("year", "2021"));
}

#[test]
fn rejects_an_empty_query() {
    assert_eq!(rejected(""), QueryError::Empty);
}

#[test]
fn rejects_a_whitespace_only_query() {
    assert_eq!(rejected("   "), QueryError::Empty);
}

#[test]
fn parses_a_match_all_query() {
    assert_eq!(parsed("*:*"), Query::MatchAll);
    assert_eq!(parsed("*"), Query::MatchAll);
}

// ---- boolean structure --------------------------------------------------

#[test]
fn parses_an_explicit_and() {
    assert_eq!(
        parsed("a AND b"),
        Query::And(vec![Query::term("a"), Query::term("b")])
    );
}

#[test]
fn parses_an_explicit_or() {
    assert_eq!(
        parsed("a OR b"),
        Query::Or(vec![Query::term("a"), Query::term("b")])
    );
}

#[test]
fn two_adjacent_terms_are_an_implicit_or() {
    // What a user typing `rust ownership` means: either word, ranked by how
    // many matched.
    assert_eq!(
        parsed("a b"),
        Query::Or(vec![Query::term("a"), Query::term("b")])
    );
}

#[test]
fn and_binds_tighter_than_or() {
    assert_eq!(
        parsed("a OR b AND c"),
        Query::Or(vec![
            Query::term("a"),
            Query::And(vec![Query::term("b"), Query::term("c")])
        ])
    );
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(
        parsed("(a OR b) AND c"),
        Query::And(vec![
            Query::Or(vec![Query::term("a"), Query::term("b")]),
            Query::term("c")
        ])
    );
}

#[test]
fn parses_a_negation() {
    assert_eq!(parsed("NOT a"), Query::term("a").negate());
}

#[test]
fn parses_a_minus_as_a_negation() {
    assert_eq!(parsed("-a"), Query::term("a").negate());
}

#[test]
fn parses_a_bang_as_a_negation() {
    assert_eq!(parsed("!a"), Query::term("a").negate());
}

#[test]
fn parses_a_negated_clause_inside_an_and() {
    assert_eq!(
        parsed("a AND NOT b"),
        Query::And(vec![Query::term("a"), Query::term("b").negate()])
    );
}

#[test]
fn a_leading_plus_is_accepted_and_ignored() {
    // The requirement is read from the AND/OR structure instead.
    assert_eq!(parsed("+rust"), Query::term("rust"));
}

#[test]
fn flattens_nested_nodes_of_the_same_kind() {
    assert_eq!(
        parsed("a AND b AND c"),
        Query::And(vec![Query::term("a"), Query::term("b"), Query::term("c")])
    );
}

#[test]
fn rejects_an_unbalanced_open_paren() {
    assert_eq!(rejected("(a"), QueryError::UnbalancedParens);
}

#[test]
fn rejects_an_unbalanced_close_paren() {
    assert_eq!(rejected("a)"), QueryError::UnbalancedParens);
}

#[test]
fn rejects_a_dangling_operator() {
    assert!(parse("a AND").is_err());
    assert!(parse("OR b").is_err());
}

#[test]
fn rejects_nesting_past_the_depth_limit() {
    let deep = format!(
        "{}a{}",
        "(".repeat(MAX_DEPTH + 5),
        ")".repeat(MAX_DEPTH + 5)
    );
    assert!(matches!(parse(&deep), Err(QueryError::TooDeep { .. })));
}

// ---- phrases ------------------------------------------------------------

#[test]
fn parses_a_phrase() {
    assert_eq!(
        parsed("\"zero cost\""),
        Query::Phrase {
            field: None,
            terms: vec!["zero".to_string(), "cost".to_string()],
            slop: 0,
        }
    );
}

#[test]
fn parses_a_field_scoped_phrase() {
    assert!(matches!(
        parsed("title:\"zero cost\""),
        Query::Phrase { field: Some(_), .. }
    ));
}

#[test]
fn parses_a_phrase_with_slop() {
    assert!(matches!(parsed("\"a b\"~2"), Query::Phrase { slop: 2, .. }));
}

#[test]
fn a_single_word_phrase_becomes_a_term() {
    // Planning it as a phrase would demand positions the field may not have.
    assert_eq!(parsed("\"rust\""), Query::term("rust"));
}

#[test]
fn an_empty_phrase_matches_nothing() {
    assert_eq!(parsed("\"\""), Query::MatchNone);
}

#[test]
fn a_phrase_collapses_repeated_whitespace() {
    assert!(matches!(
        parsed("\"a    b\""),
        Query::Phrase { ref terms, .. } if terms.len() == 2
    ));
}

// ---- fuzzy and prefix ---------------------------------------------------

#[test]
fn parses_a_fuzzy_term_with_an_explicit_distance() {
    assert_eq!(
        parsed("rust~1"),
        Query::Fuzzy {
            field: None,
            text: "rust".to_string(),
            distance: 1,
        }
    );
}

#[test]
fn a_bare_tilde_uses_the_length_based_default() {
    // What a user means by "about this word". The budget scales with length:
    // two edits on a three-letter word would match almost anything.
    assert!(matches!(
        parsed("ownership~"),
        Query::Fuzzy { distance: 2, .. }
    ));
    assert!(matches!(parsed("cat~"), Query::Fuzzy { distance: 1, .. }));
    assert!(matches!(parsed("go~"), Query::Fuzzy { distance: 0, .. }));
}

#[test]
fn rejects_a_fuzzy_distance_above_the_maximum() {
    // Beyond two edits a fuzzy match stops correcting typos and starts
    // matching unrelated words.
    assert!(matches!(
        parse("rust~5"),
        Err(QueryError::BadFuzzyDistance { .. })
    ));
}

#[test]
fn parses_a_prefix_term() {
    assert_eq!(
        parsed("own*"),
        Query::Prefix {
            field: None,
            text: "own".to_string(),
        }
    );
}

#[test]
fn parses_a_field_scoped_prefix() {
    assert!(matches!(
        parsed("title:own*"),
        Query::Prefix { field: Some(_), .. }
    ));
}

#[test]
fn a_bare_field_star_means_any_value() {
    assert_eq!(
        parsed("title:*"),
        Query::Prefix {
            field: Some("title".to_string()),
            text: String::new(),
        }
    );
}

// ---- boosts -------------------------------------------------------------

#[test]
fn parses_a_boost() {
    assert_eq!(parsed("rust^2"), Query::term("rust").boost(2.0));
}

#[test]
fn parses_a_fractional_boost() {
    assert!(matches!(parsed("rust^0.5"), Query::Boost { factor, .. } if factor == 0.5));
}

#[test]
fn parses_a_boost_on_a_group() {
    assert!(matches!(parsed("(a OR b)^3"), Query::Boost { .. }));
}

#[test]
fn a_boost_of_one_is_simplified_away() {
    assert_eq!(parsed("rust^1"), Query::term("rust"));
}

#[test]
fn rejects_a_boost_without_a_number() {
    assert!(parse("rust^").is_err());
}

// ---- ranges -------------------------------------------------------------

#[test]
fn parses_an_inclusive_range() {
    assert_eq!(
        parsed("year:[2000 TO 2020]"),
        Query::Range {
            field: "year".to_string(),
            lower: Some(2000.0),
            upper: Some(2020.0),
            include_lower: true,
            include_upper: true,
        }
    );
}

#[test]
fn parses_an_exclusive_range() {
    assert!(matches!(
        parsed("year:{2000 TO 2020}"),
        Query::Range {
            include_lower: false,
            include_upper: false,
            ..
        }
    ));
}

#[test]
fn parses_a_half_open_range() {
    assert!(matches!(
        parsed("year:[2000 TO 2020}"),
        Query::Range {
            include_lower: true,
            include_upper: false,
            ..
        }
    ));
}

#[test]
fn parses_an_unbounded_range_end() {
    assert!(matches!(
        parsed("year:[2000 TO *]"),
        Query::Range { upper: None, .. }
    ));
}

#[test]
fn parses_a_negative_range_bound() {
    assert!(matches!(
        parsed("score:[-10 TO 10]"),
        Query::Range { lower: Some(l), .. } if (l + 10.0).abs() < f64::EPSILON
    ));
}

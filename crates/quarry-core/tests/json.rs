//! The hand-rolled JSON reader and writer.
//!
//! A parser that reads network input is a security surface, so the invalid
//! cases matter as much as the valid ones. The rejections below are all things
//! a lenient parser would accept and a strict one must not.

use quarry_core::json::{parse, Json, JsonBuilder, MAX_DEPTH};

fn parses(input: &str) -> Json {
    parse(input).unwrap_or_else(|err| panic!("{input} should parse, got {err}"))
}

fn rejects(input: &str) {
    assert!(parse(input).is_err(), "{input} should have been rejected");
}

// ---- scalars ------------------------------------------------------------

#[test]
fn parses_null() {
    assert_eq!(parses("null"), Json::Null);
}

#[test]
fn parses_booleans() {
    assert_eq!(parses("true"), Json::Bool(true));
    assert_eq!(parses("false"), Json::Bool(false));
}

#[test]
fn parses_a_zero() {
    assert_eq!(parses("0"), Json::Number(0.0));
}

#[test]
fn parses_a_positive_integer() {
    assert_eq!(parses("42"), Json::Number(42.0));
}

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

#[test]
fn parses_a_negative_integer() {
    assert_eq!(parses("-17"), Json::Number(-17.0));
}

#[test]
fn parses_a_fraction() {
    assert_eq!(parses("3.5"), Json::Number(3.5));
}

#[test]
fn parses_a_negative_fraction() {
    assert_eq!(parses("-0.25"), Json::Number(-0.25));
}

#[test]
fn parses_an_exponent() {
    assert_eq!(parses("1e3"), Json::Number(1000.0));
    assert_eq!(parses("1E3"), Json::Number(1000.0));
}

#[test]
fn parses_a_signed_exponent() {
    assert_eq!(parses("1e+3"), Json::Number(1000.0));
    assert_eq!(parses("1e-3"), Json::Number(0.001));
}

#[test]
fn parses_a_fraction_with_an_exponent() {
    assert_eq!(parses("1.5e2"), Json::Number(150.0));
}

#[test]
fn parses_a_large_number() {
    assert_eq!(parses("1e308").as_f64().map(f64::is_finite), Some(true));
}

#[test]
fn rejects_a_leading_plus() {
    rejects("+1");
}

#[test]
fn rejects_a_leading_zero_before_more_digits() {
    // JSON forbids `01`, and accepting it invites octal confusion.
    rejects("01");
    rejects("-01");
}

#[test]
fn rejects_a_bare_minus() {
    rejects("-");
}

#[test]
fn rejects_a_trailing_decimal_point() {
    rejects("1.");
}

#[test]
fn rejects_a_leading_decimal_point() {
    rejects(".5");
}

#[test]
fn rejects_an_empty_exponent() {
    rejects("1e");
    rejects("1e+");
}

#[test]
fn rejects_hexadecimal() {
    rejects("0x10");
}

#[test]
fn rejects_infinity_and_nan_spellings() {
    // Neither is expressible in JSON, and accepting them would produce values
    // that cannot be written back out.
    rejects("Infinity");
    rejects("NaN");
    rejects("-Infinity");
}

// ---- strings ------------------------------------------------------------

#[test]
fn parses_an_empty_string() {
    assert_eq!(parses("\"\""), Json::String(String::new()));
}

#[test]
fn parses_a_plain_string() {
    assert_eq!(parses("\"hello\""), Json::String("hello".to_string()));
}

#[test]
fn parses_an_escaped_quote() {
    assert_eq!(parses(r#""a\"b""#), Json::String("a\"b".to_string()));
}

#[test]
fn parses_an_escaped_backslash() {
    assert_eq!(parses(r#""a\\b""#), Json::String("a\\b".to_string()));
}

#[test]
fn parses_an_escaped_solidus() {
    assert_eq!(parses(r#""a\/b""#), Json::String("a/b".to_string()));
}

#[test]
fn parses_the_control_escapes() {
    assert_eq!(parses(r#""a\nb""#), Json::String("a\nb".to_string()));
    assert_eq!(parses(r#""a\tb""#), Json::String("a\tb".to_string()));
    assert_eq!(parses(r#""a\rb""#), Json::String("a\rb".to_string()));
    assert_eq!(parses(r#""a\bb""#), Json::String("a\u{08}b".to_string()));
    assert_eq!(parses(r#""a\fb""#), Json::String("a\u{0c}b".to_string()));
}

#[test]
fn parses_a_unicode_escape() {
    assert_eq!(parses(r#""A""#), Json::String("A".to_string()));
}

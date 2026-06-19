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

#[test]
fn parses_a_unicode_escape_above_ascii() {
    assert_eq!(parses(r#""é""#), Json::String("é".to_string()));
}

#[test]
fn parses_a_surrogate_pair_into_one_code_point() {
    // An astral character is spelled as two escapes and must be joined.
    assert_eq!(parses(r#""😀""#), Json::String("😀".to_string()));
}

#[test]
fn rejects_a_lone_high_surrogate() {
    rejects(r#""\ud83d""#);
}

#[test]
fn rejects_a_lone_low_surrogate() {
    rejects(r#""\ude00""#);
}

#[test]
fn rejects_a_high_surrogate_followed_by_a_non_surrogate() {
    rejects(r#""\ud83dA""#);
}

#[test]
fn rejects_a_short_unicode_escape() {
    rejects(r#""\u00""#);
}

#[test]
fn rejects_a_non_hex_unicode_escape() {
    rejects(r#""\uZZZZ""#);
}

#[test]
fn rejects_an_unknown_escape() {
    rejects(r#""\q""#);
}

#[test]
fn rejects_an_unterminated_string() {
    rejects("\"open");
}

#[test]
fn rejects_a_raw_control_character_in_a_string() {
    // Must be escaped; accepting it silently would round-trip wrongly.
    rejects("\"a\nb\"");
    rejects("\"a\u{01}b\"");
}

#[test]
fn parses_multibyte_characters_in_a_string() {
    assert_eq!(parses("\"日本語\""), Json::String("日本語".to_string()));
}

#[test]
fn parses_an_emoji_in_a_string() {
    assert_eq!(parses("\"🙂\""), Json::String("🙂".to_string()));
}

#[test]
fn rejects_single_quoted_strings() {
    rejects("'hello'");
}

// ---- arrays -------------------------------------------------------------

#[test]
fn parses_an_empty_array() {
    assert_eq!(parses("[]"), Json::Array(vec![]));
}

#[test]
fn parses_an_array_of_numbers() {
    assert_eq!(
        parses("[1,2,3]"),
        Json::Array(vec![
            Json::Number(1.0),
            Json::Number(2.0),
            Json::Number(3.0)
        ])
    );
}

#[test]
fn parses_an_array_with_mixed_types() {
    let parsed = parses(r#"[1,"two",true,null]"#);
    assert_eq!(parsed.as_array().map(<[Json]>::len), Some(4));
}

#[test]
fn parses_a_nested_array() {
    let parsed = parses("[[1],[2]]");
    assert_eq!(parsed.as_array().map(<[Json]>::len), Some(2));
}

#[test]
fn parses_an_array_with_whitespace_everywhere() {
    assert_eq!(parses(" [ 1 , 2 ] ").as_array().map(<[Json]>::len), Some(2));
}

#[test]
fn rejects_a_trailing_comma_in_an_array() {
    rejects("[1,2,]");
}

#[test]
fn rejects_a_leading_comma_in_an_array() {
    rejects("[,1]");
}

#[test]
fn rejects_an_unterminated_array() {
    rejects("[1,2");
}

#[test]
fn rejects_a_missing_comma_in_an_array() {
    rejects("[1 2]");
}

// ---- objects ------------------------------------------------------------

#[test]
fn parses_an_empty_object() {
    assert_eq!(
        parses("{}")
            .as_object()
            .map(std::collections::BTreeMap::len),
        Some(0)
    );
}

#[test]
fn parses_an_object_with_one_pair() {
    let parsed = parses(r#"{"a":1}"#);
    assert_eq!(parsed.get("a"), Some(&Json::Number(1.0)));
}

#[test]
fn parses_an_object_with_several_pairs() {
    let parsed = parses(r#"{"a":1,"b":2,"c":3}"#);
    assert_eq!(
        parsed.as_object().map(std::collections::BTreeMap::len),
        Some(3)
    );
}

#[test]
fn parses_a_nested_object() {
    let parsed = parses(r#"{"outer":{"inner":"value"}}"#);
    assert_eq!(
        parsed
            .get("outer")
            .and_then(|o| o.get("inner"))
            .and_then(Json::as_str),
        Some("value")
    );
}

#[test]
fn parses_an_object_with_whitespace_everywhere() {
    let parsed = parses(" { \"a\" : 1 } ");
    assert_eq!(parsed.get("a"), Some(&Json::Number(1.0)));
}

#[test]
fn a_repeated_key_keeps_the_last_value() {
    // What most parsers do, and the only choice that round-trips.
    let parsed = parses(r#"{"a":1,"a":2}"#);
    assert_eq!(parsed.get("a"), Some(&Json::Number(2.0)));
}

#[test]
fn parses_an_empty_key() {
    let parsed = parses(r#"{"":1}"#);
    assert_eq!(parsed.get(""), Some(&Json::Number(1.0)));
}

#[test]
fn rejects_an_unquoted_key() {
    rejects("{a:1}");
}

#[test]
fn rejects_a_trailing_comma_in_an_object() {
    rejects(r#"{"a":1,}"#);
}

#[test]
fn rejects_a_missing_colon() {
    rejects(r#"{"a" 1}"#);
}

#[test]
fn rejects_a_missing_value() {
    rejects(r#"{"a":}"#);
}

#[test]
fn rejects_an_unterminated_object() {
    rejects(r#"{"a":1"#);
}

// ---- document-level ------------------------------------------------------

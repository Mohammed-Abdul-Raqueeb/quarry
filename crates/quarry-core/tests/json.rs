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

#[test]
fn rejects_empty_input() {
    rejects("");
}

#[test]
fn rejects_only_whitespace() {
    rejects("   ");
}

#[test]
fn rejects_trailing_content_after_a_value() {
    // A doubled or truncated payload must not silently half-parse.
    rejects("1 2");
    rejects("{} {}");
    rejects(r#"{"a":1} trailing"#);
}

#[test]
fn accepts_trailing_whitespace() {
    assert_eq!(parses("1  \n\t "), Json::Number(1.0));
}

#[test]
fn rejects_a_comment() {
    rejects("1 // comment");
    rejects("/* comment */ 1");
}

#[test]
fn rejects_nesting_past_the_depth_limit() {
    // An unbounded recursive parser is a stack-overflow waiting for a hostile
    // document, so the limit is a hard rejection rather than best effort.
    let deep = format!(
        "{}1{}",
        "[".repeat(MAX_DEPTH + 5),
        "]".repeat(MAX_DEPTH + 5)
    );
    rejects(&deep);
}

#[test]
fn accepts_nesting_just_under_the_depth_limit() {
    let depth = MAX_DEPTH - 2;
    let input = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
    assert!(parse(&input).is_ok());
}

// ---- accessors -----------------------------------------------------------

#[test]
fn accessors_return_the_inner_value() {
    assert_eq!(Json::String("x".to_string()).as_str(), Some("x"));
    assert_eq!(Json::Number(1.5).as_f64(), Some(1.5));
    assert_eq!(Json::Bool(true).as_bool(), Some(true));
}

#[test]
fn accessors_return_none_for_the_wrong_type() {
    assert_eq!(Json::Number(1.0).as_str(), None);
    assert_eq!(Json::String("1".to_string()).as_f64(), None);
    assert_eq!(Json::Null.as_bool(), None);
}

#[test]
fn get_returns_none_for_a_non_object() {
    assert_eq!(Json::Number(1.0).get("a"), None);
}

#[test]
fn get_returns_none_for_a_missing_key() {
    assert_eq!(parses("{}").get("missing"), None);
}

#[test]
fn is_null_identifies_null_only() {
    assert!(Json::Null.is_null());
    assert!(!Json::Bool(false).is_null());
}

#[test]
fn type_name_describes_every_variant() {
    assert_eq!(Json::Null.type_name(), "null");
    assert_eq!(Json::Bool(true).type_name(), "boolean");
    assert_eq!(Json::Number(1.0).type_name(), "number");
    assert_eq!(Json::String(String::new()).type_name(), "string");
    assert_eq!(Json::Array(vec![]).type_name(), "array");
    assert_eq!(parses("{}").type_name(), "object");
}

// ---- writing -------------------------------------------------------------

#[test]
fn writes_null_and_booleans() {
    assert_eq!(Json::Null.to_string(), "null");
    assert_eq!(Json::Bool(true).to_string(), "true");
    assert_eq!(Json::Bool(false).to_string(), "false");
}

#[test]
fn writes_an_integer_without_a_decimal_point() {
    // Numbers are held as f64, so this is what keeps `42` from becoming `42.0`.
    assert_eq!(Json::Number(42.0).to_string(), "42");
    assert_eq!(Json::Number(-7.0).to_string(), "-7");
}

#[test]
fn writes_a_fraction() {
    assert_eq!(Json::Number(1.5).to_string(), "1.5");
}

#[test]
fn writes_a_non_finite_number_as_null() {
    // JSON cannot spell these, and emitting `NaN` would produce output no
    // parser accepts.
    assert_eq!(Json::Number(f64::NAN).to_string(), "null");
    assert_eq!(Json::Number(f64::INFINITY).to_string(), "null");
}

#[test]
fn escapes_a_quote_and_a_backslash() {
    assert_eq!(Json::String("a\"b".to_string()).to_string(), r#""a\"b""#);
    assert_eq!(Json::String("a\\b".to_string()).to_string(), r#""a\\b""#);
}

#[test]
fn escapes_the_control_characters() {
    assert_eq!(Json::String("\n".to_string()).to_string(), r#""\n""#);
    assert_eq!(Json::String("\t".to_string()).to_string(), r#""\t""#);
    assert_eq!(
        Json::String("\u{01}".to_string()).to_string(),
        r#""\u0001""#
    );
}

#[test]
fn writes_multibyte_characters_raw() {
    // The output is declared UTF-8, so escaping them would only bloat it.
    assert_eq!(Json::String("日本".to_string()).to_string(), "\"日本\"");
}

#[test]
fn writes_an_array_without_spaces() {
    assert_eq!(
        Json::Array(vec![Json::Number(1.0), Json::Number(2.0)]).to_string(),
        "[1,2]"
    );
}

#[test]
fn writes_object_keys_in_sorted_order() {
    // Sorted keys are what make the segment tests able to compare serialized
    // forms byte for byte.
    let parsed = parses(r#"{"z":1,"a":2}"#);
    assert_eq!(parsed.to_string(), r#"{"a":2,"z":1}"#);
}

// ---- round trips ---------------------------------------------------------

#[test]
fn round_trips_every_scalar() {
    for input in ["null", "true", "false", "0", "-1", "1.5", "\"text\""] {
        let once = parses(input).to_string();
        assert_eq!(
            parses(&once).to_string(),
            once,
            "{input} did not round trip"
        );
    }
}

#[test]
fn round_trips_a_nested_document() {
    let input = r#"{"a":[1,{"b":"c"}],"d":null}"#;
    assert_eq!(parses(input).to_string(), input);
}

#[test]
fn round_trips_escapes() {
    let original = Json::String("quote\" slash\\ tab\t newline\n".to_string());
    assert_eq!(parses(&original.to_string()), original);
}

#[test]
fn round_trips_a_surrogate_pair() {
    let original = Json::String("😀".to_string());
    assert_eq!(parses(&original.to_string()), original);
}

#[test]
fn round_trips_a_realistic_document() {
    let input = r#"{"body":"Ownership explained","tag":"rust","title":"Rust","year":2021}"#;
    assert_eq!(parses(input).to_string(), input);
}

// ---- JsonBuilder ---------------------------------------------------------

#[test]
fn builder_produces_an_empty_object() {
    assert_eq!(JsonBuilder::new().build().to_string(), "{}");
}

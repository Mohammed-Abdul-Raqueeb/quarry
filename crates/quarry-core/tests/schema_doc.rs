//! Schema definition and document construction.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::doc::{format_number, Document, Value};
use quarry_core::json::parse;
use quarry_core::schema::{FieldOptions, FieldType, Schema};

fn basic_schema() -> Schema {
    Schema::builder()
        .text("title")
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(|b| b.numeric("year"))
        .and_then(quarry_core::schema::SchemaBuilder::build)
        .expect("schema should build")
}

// ---- FieldType ----------------------------------------------------------

#[test]
fn only_text_is_analyzed() {
    assert!(FieldType::Text.is_analyzed());
    assert!(!FieldType::Keyword.is_analyzed());
    assert!(!FieldType::Numeric.is_analyzed());
}

#[test]
fn field_type_names_round_trip() {
    for ty in [FieldType::Text, FieldType::Keyword, FieldType::Numeric] {
        assert_eq!(FieldType::parse(ty.as_str()).ok(), Some(ty));
    }
}

#[test]
fn field_type_rejects_an_unknown_name() {
    assert!(FieldType::parse("vector").is_err());
}

#[test]
fn field_type_parsing_is_case_sensitive() {
    // The on-disk form is lowercase; accepting other spellings would make two
    // schema files that look different compare as equal.
    assert!(FieldType::parse("Text").is_err());
}

// ---- building -----------------------------------------------------------

#[test]
fn builds_a_schema_with_several_fields() {
    assert_eq!(basic_schema().len(), 4);
}

#[test]
fn assigns_ids_in_declaration_order() {
    let schema = basic_schema();
    assert_eq!(schema.field("title").map(|f| f.id), Some(0));
    assert_eq!(schema.field("year").map(|f| f.id), Some(3));
}

#[test]
fn rejects_an_empty_schema() {
    assert!(Schema::builder().build().is_err());
}

#[test]
fn rejects_a_duplicate_field_name() {
    let result = Schema::builder()
        .text("title")
        .and_then(|b| b.text("title"));
    assert!(result.is_err());
}

#[test]
fn rejects_an_empty_field_name() {
    assert!(Schema::builder().text("").is_err());
}

#[test]
fn rejects_a_reserved_underscore_prefix() {
    // Reserved so the query parser can treat `_all` specially without a real
    // field being able to shadow it.
    assert!(Schema::builder().text("_all").is_err());
}

#[test]
fn text_fields_get_positions_by_default() {
    let schema = basic_schema();
    assert_eq!(
        schema.field("title").map(|f| f.options.positions),
        Some(true)
    );
}

#[test]
fn keyword_fields_do_not_get_positions() {
    // Positions roughly double a posting list, and a keyword is one token.
    let schema = basic_schema();
    assert_eq!(
        schema.field("tag").map(|f| f.options.positions),
        Some(false)
    );
}

#[test]
fn a_boosted_text_field_records_its_boost() {
    let schema = Schema::builder()
        .text_boosted("title", 2.5)
        .and_then(quarry_core::schema::SchemaBuilder::build)
        .expect("should build");

    assert_eq!(schema.field("title").map(|f| f.options.boost), Some(2.5));
}

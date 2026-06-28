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

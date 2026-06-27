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

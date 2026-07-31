//! `quarry index` — build or extend an index.

use crate::args::Args;
use crate::output::{human_bytes, human_duration, thousands, Style};
use quarry_core::index::WriterConfig;
use quarry_core::json::{self, Json};
use quarry_core::schema::{FieldType, Schema, SchemaBuilder};
use quarry_core::{Error, IndexDir, IndexWriter, Result};
use std::io::Read;
use std::time::Instant;

/// Runs the command.
///
/// Reads newline-delimited JSON, one document per line, from a file or stdin.
/// NDJSON rather than a JSON array because an array has to be fully parsed
/// before the first document can be indexed, which puts a ceiling on input size
/// for no benefit.
pub fn run(args: &Args, style: Style) -> Result<()> {
    let path = args.require("index", Some("i")).map_err(Error::schema)?;
    let input = args.value("input", Some("f"));
    let batch_size = args.number("batch", None, 10_000).map_err(Error::schema)?;

    let mut dir = if std::path::Path::new(path).join("schema.json").exists() {
        IndexDir::open(path)?
    } else {
        let schema = parse_schema_spec(args.require("schema", Some("s")).map_err(Error::schema)?)?;
        println!(
            "{} index at {} with {} fields",
            style.green("created"),
            style.cyan(path),
            schema.len()
        );
        IndexDir::create(path, &schema)?
    };

    let schema = dir.schema().clone();
    let mut writer = IndexWriter::new(schema).with_config(WriterConfig {
        max_buffered_docs: batch_size as u32,
        ..WriterConfig::default()
    });

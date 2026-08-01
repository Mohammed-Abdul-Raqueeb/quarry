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

    let raw = read_input(input)?;
    let started = Instant::now();

    let mut indexed = 0u64;
    let mut rejected = 0u64;
    let mut segments = 0u32;

    for (line_number, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match index_line(&mut writer, trimmed) {
            Ok(()) => indexed += 1,
            Err(err) => {
                rejected += 1;
                // One malformed line should not abandon the other ten thousand,
                // but it must be reported with its line number to be fixable.
                eprintln!(
                    "{} line {}: {err}",
                    style.yellow("skipped"),
                    line_number + 1
                );
            }
        }

        // Flushing mid-run bounds memory, which is the whole reason the writer
        // exposes a threshold rather than deciding for itself.
        if writer.should_flush() {
            if let Some(segment) = writer.flush() {
                dir.append(segment)?;
                segments += 1;
            }
        }
    }

    if let Some(segment) = writer.flush() {
        dir.append(segment)?;
        segments += 1;
    }

    let elapsed = started.elapsed();

    println!();
    println!("{}", style.bold("Indexed"));
    println!("  documents   {}", thousands(indexed));
    if rejected > 0 {
        println!("  rejected    {}", style.yellow(&thousands(rejected)));
    }
    println!("  segments    {segments}");
    println!("  on disk     {}", human_bytes(dir.size_on_disk()?));
    println!("  took        {}", human_duration(elapsed));

    if indexed > 0 && elapsed.as_secs_f64() > 0.0 {
        let rate = indexed as f64 / elapsed.as_secs_f64();
        println!("  rate        {} docs/s", thousands(rate as u64));
    }

    if dir.segments().len() >= 4 {
        println!();
        println!(
            "{} {} segments; `quarry merge --index {}` will speed up queries",
            style.dim("hint:"),
            dir.segments().len(),
            path
        );
    }

    Ok(())
}

fn index_line(writer: &mut IndexWriter, line: &str) -> Result<()> {
    let json = json::parse(line)?;
    writer.add_json(&json)?;
    Ok(())
}

fn read_input(path: Option<&str>) -> Result<String> {
    match path {
        Some("-") | None => {
            let mut buffer = String::new();
            std::io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer)
        }
        Some(path) => Ok(std::fs::read_to_string(path)?),
    }
}

/// Parses a schema specification.
///
/// Either a path to a JSON schema file, or a compact inline form:
/// `title:text,body:text,tag:keyword,year:numeric`. The inline form exists
/// because needing a file to index one NDJSON file is friction out of
/// proportion to the task.
pub fn parse_schema_spec(spec: &str) -> Result<Schema> {
    // Case-insensitive: a path typed as SCHEMA.JSON is still a schema file.
    if spec.to_ascii_lowercase().ends_with(".json") {
        let raw = std::fs::read_to_string(spec)?;
        return quarry_core::dir::schema_from_json(&json::parse(&raw)?);
    }

    let mut builder: SchemaBuilder = Schema::builder();

    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (name, ty) = match part.split_once(':') {
            Some((name, ty)) => (name.trim(), ty.trim()),
            // A bare name defaults to text, which is what it is most of the time.
            None => (part, "text"),
        };

        builder = match FieldType::parse(ty)? {
            FieldType::Text => builder.text(name)?,
            FieldType::Keyword => builder.keyword(name)?,
            FieldType::Numeric => builder.numeric(name)?,
        };
    }

    builder.build()
}

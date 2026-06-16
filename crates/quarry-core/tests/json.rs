//! The hand-rolled JSON reader and writer.
//!
//! A parser that reads network input is a security surface, so the invalid
//! cases matter as much as the valid ones. The rejections below are all things
//! a lenient parser would accept and a strict one must not.

use quarry_core::json::{parse, Json, JsonBuilder, MAX_DEPTH};

fn parses(input: &str) -> Json {
    parse(input).unwrap_or_else(|err| panic!("{input} should parse, got {err}"))
}

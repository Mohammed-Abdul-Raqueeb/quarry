//! A small JSON reader and writer.
//!
//! Quarry takes documents as JSON and the server answers in JSON, which is the
//! only reason this exists. `serde_json` would be the obvious dependency, but
//! the engine ships with none — and the subset needed here is a few hundred
//! lines, with the recursion depth capped so a hostile document cannot blow the
//! stack.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

/// Deepest nesting accepted. Well past anything a real document needs.
pub const MAX_DEPTH: usize = 64;

/// A JSON value.
///
/// Objects keep their keys sorted in a [`BTreeMap`] rather than preserving
/// input order: round-tripping then produces byte-identical output, which makes
/// the segment tests able to compare serialized forms directly.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// Any JSON number, held as `f64`.
    Number(f64),
    /// A string, already unescaped.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object with sorted keys.
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// The value at `key`, if this is an object that has it.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    /// The string inside, if this is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The number inside, if this is a number.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The boolean inside, if this is a boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The entries, if this is an object.
    #[must_use]
    pub fn as_object(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }

    /// Whether this is [`Json::Null`].
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The name of this value's type, for error messages.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(true) => f.write_str("true"),
            Self::Bool(false) => f.write_str("false"),
            Self::Number(n) => write_number(f, *n),
            Self::String(s) => write_string(f, s),
            Self::Array(items) => {
                f.write_char('[')?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_char(',')?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_char(']')
            }
            Self::Object(map) => {
                f.write_char('{')?;
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        f.write_char(',')?;
                    }
                    write_string(f, key)?;
                    f.write_char(':')?;
                    write!(f, "{value}")?;
                }
                f.write_char('}')
            }
        }
    }
}

/// Writes a number without a trailing `.0`, so integers round-trip as integers.
fn write_number(f: &mut fmt::Formatter<'_>, n: f64) -> fmt::Result {
    // JSON has no way to spell a non-finite number, and emitting `NaN` would
    // produce output no parser accepts. Null is the conventional stand-in.
    if !n.is_finite() {
        return f.write_str("null");
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        write!(f, "{}", n as i64)
    } else {
        write!(f, "{n}")
    }
}

fn write_string(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    f.write_char('"')?;
    for ch in s.chars() {
        match ch {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            '\u{08}' => f.write_str("\\b")?,
            '\u{0c}' => f.write_str("\\f")?,
            // Everything below 0x20 must be escaped; the rest can go out raw
            // because the output is declared UTF-8.
            c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
            c => f.write_char(c)?,
        }
    }
    f.write_char('"')
}

/// Parses a complete JSON document.
///
/// Trailing content after the value is an error, so a truncated or doubled
/// payload is rejected rather than silently half-read.
pub fn parse(input: &str) -> Result<Json> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        pos: 0,
        depth: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(Error::json(format!(
            "unexpected trailing input at byte {}",
            parser.pos
        )));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn expect(&mut self, want: u8) -> Result<()> {
        match self.bump() {
            Some(b) if b == want => Ok(()),
            Some(b) => Err(Error::json(format!(
                "expected {:?} but found {:?} at byte {}",
                want as char,
                b as char,
                self.pos - 1
            ))),
            None => Err(Error::json(format!(
                "expected {:?} but input ended",
                want as char
            ))),
        }
    }

    fn parse_value(&mut self) -> Result<Json> {
        if self.depth >= MAX_DEPTH {
            return Err(Error::json(format!("nesting deeper than {MAX_DEPTH}")));
        }

        match self.peek() {
            Some(b'n') => self.parse_literal("null", Json::Null),
            Some(b't') => self.parse_literal("true", Json::Bool(true)),
            Some(b'f') => self.parse_literal("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b) => Err(Error::json(format!(
                "unexpected {:?} at byte {}",
                b as char, self.pos
            ))),
            None => Err(Error::json("unexpected end of input")),
        }
    }

    fn parse_literal(&mut self, word: &str, value: Json) -> Result<Json> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(Error::json(format!("expected {word} at byte {}", self.pos)))
        }
    }

    fn parse_array(&mut self) -> Result<Json> {
        self.expect(b'[')?;
        self.depth += 1;
        let mut items = Vec::new();

        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Json::Array(items));
        }

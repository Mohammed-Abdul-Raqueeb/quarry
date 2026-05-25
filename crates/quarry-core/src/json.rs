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

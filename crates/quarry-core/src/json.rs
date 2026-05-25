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

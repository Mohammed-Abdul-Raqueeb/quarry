//! Documents as they enter the index.

use crate::error::{Error, Result};
use crate::json::Json;
use crate::schema::{FieldId, FieldType, Schema};
use std::collections::BTreeMap;

/// A document's dense, index-local identifier.
///
/// Assigned in insertion order within a segment, which is what lets posting
/// lists be gap-encoded. It is not stable across a merge — the external id is
/// whatever the caller stored in a keyword field.
pub type DocId = u32;

/// A field value on an incoming document.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Prose, to be analyzed.
    Text(String),
    /// An exact token.
    Keyword(String),
    /// A number.
    Number(f64),
}

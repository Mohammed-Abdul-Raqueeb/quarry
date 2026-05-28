//! Field definitions.
//!
//! A schema is fixed before indexing starts. Quarry is not schemaless on
//! purpose: knowing a field is text rather than a keyword at index time is what
//! decides whether it gets analyzed, whether positions are stored, and how it
//! is scored. Deciding that per document instead produces an index where the
//! same query means different things for different documents.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// A field's position in the schema. Stored in postings instead of its name.
pub type FieldId = u16;

/// How a field's value is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Analyzed into terms. Use for prose.
    Text,
    /// Indexed whole, unanalyzed. Use for ids, tags, enum values.
    Keyword,
    /// Indexed for range queries and sorting.
    Numeric,
}

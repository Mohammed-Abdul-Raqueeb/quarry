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

impl FieldType {
    /// Whether values of this type go through the analyzer.
    #[must_use]
    pub fn is_analyzed(self) -> bool {
        matches!(self, Self::Text)
    }

    /// Short name used in the on-disk schema and in error messages.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Keyword => "keyword",
            Self::Numeric => "numeric",
        }
    }

    /// Parses what [`FieldType::as_str`] produced.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "text" => Ok(Self::Text),
            "keyword" => Ok(Self::Keyword),
            "numeric" => Ok(Self::Numeric),
            other => Err(Error::schema(format!("unknown field type: {other}"))),
        }
    }
}

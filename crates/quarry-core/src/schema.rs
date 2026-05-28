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

/// Per-field indexing options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldOptions {
    /// Whether the field can be searched.
    pub indexed: bool,
    /// Whether the original value is kept for retrieval and highlighting.
    pub stored: bool,
    /// Whether term positions are recorded. Required for phrase queries.
    pub positions: bool,
    /// Multiplier applied to this field's contribution to a score.
    pub boost: f32,
}

impl Default for FieldOptions {
    fn default() -> Self {
        Self {
            indexed: true,
            stored: true,
            // Positions roughly double the size of a posting list, so they are
            // opt-in; `text_field` turns them on because phrase queries are the
            // common case for prose.
            positions: false,
            boost: 1.0,
        }
    }
}

/// One field in the schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Dense id assigned in declaration order.
    pub id: FieldId,
    /// Name used in documents and queries.
    pub name: String,
    /// How the value is treated.
    pub ty: FieldType,
    /// Indexing options.
    pub options: FieldOptions,
}

/// The set of fields an index accepts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    fields: Vec<Field>,
    by_name: HashMap<String, FieldId>,
}

impl Schema {
    /// Starts building a schema.
    #[must_use]
    pub fn builder() -> SchemaBuilder {
        SchemaBuilder::default()
    }

    /// Every field, in declaration order.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// How many fields are defined.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether no fields are defined.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// The field with this name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.by_name
            .get(name)
            .and_then(|id| self.fields.get(*id as usize))
    }

    /// The field with this id.
    #[must_use]
    pub fn field_by_id(&self, id: FieldId) -> Option<&Field> {
        self.fields.get(id as usize)
    }

    /// The id of the field with this name.
    pub fn field_id(&self, name: &str) -> Result<FieldId> {
        self.by_name
            .get(name)
            .copied()
            .ok_or_else(|| Error::UnknownField(name.to_string()))
    }

    /// Fields that are searched when a query names no field.
    ///
    /// Every indexed text field, which is what a user expects from a bare term.
    #[must_use]
    pub fn default_search_fields(&self) -> Vec<FieldId> {
        self.fields
            .iter()
            .filter(|f| f.options.indexed && f.ty == FieldType::Text)
            .map(|f| f.id)
            .collect()
    }

    /// Fields whose values are kept for retrieval.
    #[must_use]
    pub fn stored_fields(&self) -> Vec<FieldId> {
        self.fields
            .iter()
            .filter(|f| f.options.stored)
            .map(|f| f.id)
            .collect()
    }
}

/// Accumulates fields, then produces a [`Schema`].
#[derive(Debug, Default)]
pub struct SchemaBuilder {
    fields: Vec<Field>,
    by_name: HashMap<String, FieldId>,
}

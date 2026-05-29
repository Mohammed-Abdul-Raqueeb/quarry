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

impl Value {
    /// The value as text, for analysis or storage.
    #[must_use]
    pub fn as_text(&self) -> String {
        match self {
            Self::Text(s) | Self::Keyword(s) => s.clone(),
            Self::Number(n) => format_number(*n),
        }
    }

    /// The value as a number, if it is one.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            // A keyword that happens to look numeric is still a keyword; only
            // an actual number is one, so range queries cannot silently
            // succeed on a mistyped field.
            _ => None,
        }
    }

    /// Whether this value is compatible with `ty`.
    #[must_use]
    pub fn matches(&self, ty: FieldType) -> bool {
        matches!(
            (self, ty),
            (Self::Text(_), FieldType::Text)
                | (Self::Keyword(_), FieldType::Keyword)
                | (Self::Number(_), FieldType::Numeric)
        )
    }
}

/// Renders a number the way it is stored and shown.
#[must_use]
pub fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// A document ready to be indexed.
///
/// Fields are keyed by [`FieldId`], so a document is already validated against
/// a schema by the time it exists — there is no way to build one holding a field
/// the schema does not define.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    fields: BTreeMap<FieldId, Vec<Value>>,
}

impl Document {
    /// An empty document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a value, keeping any already present for the field.
    ///
    /// Repeating a field is how a multi-valued field is expressed: three `tag`
    /// values index as three keywords on one document.
    pub fn add(&mut self, field: FieldId, value: Value) {
        self.fields.entry(field).or_default().push(value);
    }

    /// Replaces every value for a field.
    pub fn set(&mut self, field: FieldId, value: Value) {
        self.fields.insert(field, vec![value]);
    }

    /// Values for a field.
    #[must_use]
    pub fn values(&self, field: FieldId) -> &[Value] {
        self.fields.get(&field).map_or(&[], Vec::as_slice)
    }

    /// The first value for a field.
    #[must_use]
    pub fn value(&self, field: FieldId) -> Option<&Value> {
        self.fields.get(&field).and_then(|v| v.first())
    }

    /// Every populated field and its values.
    pub fn iter(&self) -> impl Iterator<Item = (FieldId, &[Value])> {
        self.fields.iter().map(|(id, v)| (*id, v.as_slice()))
    }

    /// How many fields are populated.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the document has no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Builds a document from JSON, validating against `schema`.
    ///
    /// A JSON type that does not match the field's declared type is an error
    /// rather than a coercion. Quietly turning `"42"` into a number is how an
    /// index ends up with a numeric field that cannot be range-queried
    /// consistently.
    pub fn from_json(schema: &Schema, json: &Json) -> Result<Self> {
        let object = json.as_object().ok_or_else(|| {
            Error::schema(format!(
                "a document must be an object, got {}",
                json.type_name()
            ))
        })?;

        let mut doc = Self::new();

        for (name, value) in object {
            let field = schema
                .field(name)
                .ok_or_else(|| Error::UnknownField(name.clone()))?;

            match value {
                Json::Array(items) => {
                    for item in items {
                        doc.add(field.id, value_from_json(field.ty, name, item)?);
                    }
                }
                single => doc.add(field.id, value_from_json(field.ty, name, single)?),
            }
        }

        Ok(doc)
    }

    /// Renders the document as JSON, naming fields from `schema`.
    #[must_use]
    pub fn to_json(&self, schema: &Schema) -> Json {
        let mut map = BTreeMap::new();

        for (id, values) in self.iter() {
            let Some(field) = schema.field_by_id(id) else {
                continue;
            };

            let rendered = if values.len() == 1 {
                value_to_json(&values[0])
            } else {
                Json::Array(values.iter().map(value_to_json).collect())
            };

            map.insert(field.name.clone(), rendered);
        }

        Json::Object(map)
    }
}

//! Storage for the original field values.
//!
//! The inverted index can tell you *which* documents match but not what they
//! said — analysis is lossy, so `run` cannot be turned back into `running`.
//! Anything shown to a user, and anything highlighted, comes from here.

use super::blob::{BlobReader, BlobWriter};
use crate::doc::{DocId, Document, Value};
use crate::error::{Error, Result};
use crate::schema::{FieldId, FieldType, Schema};
use std::collections::BTreeMap;

/// Stored values, keyed by document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocStore {
    docs: BTreeMap<DocId, Vec<(FieldId, Value)>>,
}

impl DocStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many documents are stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// Whether nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Stores the values of `doc` that the schema marks as stored.
    ///
    /// Filtering here rather than at read time means an unstored field never
    /// reaches the disk, which is the point of the option.
    pub fn put(&mut self, id: DocId, doc: &Document, schema: &Schema) {
        let mut values = Vec::new();

        for (field, field_values) in doc.iter() {
            let Some(definition) = schema.field_by_id(field) else {
                continue;
            };
            if !definition.options.stored {
                continue;
            }
            for value in field_values {
                values.push((field, value.clone()));
            }
        }

        if !values.is_empty() {
            self.docs.insert(id, values);
        }
    }

    /// Inserts already-filtered values under a new id.
    ///
    /// Used by the merger, which is re-keying values that were filtered against
    /// the schema when they were first stored.
    pub fn put_raw(&mut self, id: DocId, values: Vec<(FieldId, Value)>) {
        if !values.is_empty() {
            self.docs.insert(id, values);
        }
    }

    /// The stored values for a document.
    #[must_use]
    pub fn get(&self, id: DocId) -> Option<&[(FieldId, Value)]> {
        self.docs.get(&id).map(Vec::as_slice)
    }

    /// The stored value of one field of one document.
    #[must_use]
    pub fn field(&self, id: DocId, field: FieldId) -> Option<&Value> {
        self.docs
            .get(&id)?
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| v)
    }

    /// Every stored value of one field of one document.
    #[must_use]
    pub fn field_values(&self, id: DocId, field: FieldId) -> Vec<&Value> {
        self.docs
            .get(&id)
            .map(|values| {
                values
                    .iter()
                    .filter(|(f, _)| *f == field)
                    .map(|(_, v)| v)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Rebuilds a [`Document`] from what was stored.
    #[must_use]
    pub fn document(&self, id: DocId) -> Option<Document> {
        let values = self.docs.get(&id)?;
        let mut doc = Document::new();
        for (field, value) in values {
            doc.add(*field, value.clone());
        }
        Some(doc)
    }

    /// Document ids present, ascending.
    #[must_use]
    pub fn doc_ids(&self) -> Vec<DocId> {
        self.docs.keys().copied().collect()
    }

    /// Removes a document's stored values.
    pub fn remove(&mut self, id: DocId) -> bool {
        self.docs.remove(&id).is_some()
    }

    /// Writes the store.
    pub fn encode(&self, out: &mut BlobWriter) {
        out.varint(self.docs.len() as u64);

        let mut previous_id = 0u32;
        for (id, values) in &self.docs {
            // Ids ascend, so gaps keep the common case to one byte.
            out.varint(u64::from(*id - previous_id));
            previous_id = *id;

            out.varint(values.len() as u64);
            for (field, value) in values {
                out.varint(u64::from(*field));
                encode_value(out, value);
            }
        }
    }

    /// Reads what [`DocStore::encode`] wrote.
    pub fn decode(input: &mut BlobReader<'_>) -> Result<Self> {
        let count = input.varint()? as usize;
        let mut docs = BTreeMap::new();
        let mut id = 0u32;

        for _ in 0..count {
            let gap = input.varint_u32()?;
            id = id
                .checked_add(gap)
                .ok_or_else(|| Error::corrupt("stored doc id overflows u32"))?;

            let value_count = input.varint()? as usize;
            let mut values = Vec::with_capacity(value_count.min(256));

            for _ in 0..value_count {
                let field = u16::try_from(input.varint()?)
                    .map_err(|_| Error::corrupt("stored field id does not fit in u16"))?;
                values.push((field, decode_value(input)?));
            }

            docs.insert(id, values);
        }

        Ok(Self { docs })
    }

    /// Rough footprint in bytes, for deciding when to flush.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        self.docs
            .values()
            .map(|values| {
                values
                    .iter()
                    .map(|(_, v)| match v {
                        Value::Text(s) | Value::Keyword(s) => s.len() + 24,
                        Value::Number(_) => 16,
                    })
                    .sum::<usize>()
                    + 32
            })
            .sum()
    }
}

/// Tag byte identifying a value's variant on disk.
const TAG_TEXT: u8 = 1;
const TAG_KEYWORD: u8 = 2;
const TAG_NUMBER: u8 = 3;

fn encode_value(out: &mut BlobWriter, value: &Value) {
    match value {
        Value::Text(s) => {
            out.u8(TAG_TEXT);
            out.string(s);
        }
        Value::Keyword(s) => {
            out.u8(TAG_KEYWORD);
            out.string(s);
        }
        Value::Number(n) => {
            out.u8(TAG_NUMBER);
            // Raw bits rather than a decimal rendering, so the value survives a
            // round trip exactly.
            out.u64(n.to_bits());
        }
    }
}

fn decode_value(input: &mut BlobReader<'_>) -> Result<Value> {
    match input.u8()? {
        TAG_TEXT => Ok(Value::Text(input.string()?.to_string())),
        TAG_KEYWORD => Ok(Value::Keyword(input.string()?.to_string())),
        TAG_NUMBER => Ok(Value::Number(f64::from_bits(input.u64()?))),
        tag => Err(Error::corrupt(format!("unknown value tag {tag}"))),
    }
}

/// Checks a value is the right shape for its field.
///
/// Called when reading a segment written by an older build whose schema may
/// have differed, so a type change is reported rather than silently reinterpreted.
pub fn validate(schema: &Schema, field: FieldId, value: &Value) -> Result<()> {
    let definition = schema
        .field_by_id(field)
        .ok_or_else(|| Error::corrupt(format!("stored field {field} is not in the schema")))?;

    let ok = matches!(
        (definition.ty, value),
        (FieldType::Text, Value::Text(_))
            | (FieldType::Keyword, Value::Keyword(_))
            | (FieldType::Numeric, Value::Number(_))
    );

    if ok {
        Ok(())
    } else {
        Err(Error::corrupt(format!(
            "stored value for {} is not {}",
            definition.name,
            definition.ty.as_str()
        )))
    }
}

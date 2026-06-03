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

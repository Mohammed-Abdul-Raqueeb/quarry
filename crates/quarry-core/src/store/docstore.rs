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

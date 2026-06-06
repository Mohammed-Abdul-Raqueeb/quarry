//! Building an index.

use super::inverted::InvertedIndex;
use super::segment::Segment;
use crate::doc::{DocId, Document, Value};
use crate::error::{Error, Result};
use crate::json::Json;
use crate::schema::{FieldType, Schema};
use crate::store::docstore::DocStore;
use crate::tokenize::Analyzer;

/// Documents accepted before the writer insists on a flush.
pub const DEFAULT_MAX_BUFFERED_DOCS: u32 = 50_000;
/// Approximate bytes buffered before the writer insists on a flush.
pub const DEFAULT_MAX_BUFFERED_BYTES: usize = 64 * 1024 * 1024;

/// How the writer decides when a segment is full.
#[derive(Debug, Clone, Copy)]
pub struct WriterConfig {
    /// Documents after which [`IndexWriter::should_flush`] returns true.
    pub max_buffered_docs: u32,
    /// Estimated bytes after which [`IndexWriter::should_flush`] returns true.
    pub max_buffered_bytes: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            max_buffered_docs: DEFAULT_MAX_BUFFERED_DOCS,
            max_buffered_bytes: DEFAULT_MAX_BUFFERED_BYTES,
        }
    }
}

/// Accumulates documents into one segment.
///
/// Everything is buffered in memory until [`IndexWriter::flush`]. That is what
/// makes a segment immutable and the write path lock-free, and it is why
/// `should_flush` exists: the caller decides when to pay the cost, because only
/// the caller knows whether it is mid-batch.
#[derive(Debug)]
pub struct IndexWriter {
    schema: Schema,
    analyzer: Analyzer,
    keyword_analyzer: Analyzer,
    index: InvertedIndex,
    store: DocStore,
    config: WriterConfig,
}

impl IndexWriter {
    /// A writer over `schema` with the default English analyzer.
    #[must_use]
    pub fn new(schema: Schema) -> Self {
        Self::with_analyzer(schema, Analyzer::english())
    }

    /// A writer with an explicit analyzer for text fields.
    #[must_use]
    pub fn with_analyzer(schema: Schema, analyzer: Analyzer) -> Self {
        Self {
            schema,
            analyzer,
            // Keyword fields get their own fixed pipeline: lowercasing a tag is
            // wanted, stemming it is not, so this must not follow the text
            // analyzer's configuration.
            keyword_analyzer: Analyzer::keyword(),
            index: InvertedIndex::new(),
            store: DocStore::new(),
            config: WriterConfig::default(),
        }
    }

    /// Replaces the flush thresholds.
    #[must_use]
    pub fn with_config(mut self, config: WriterConfig) -> Self {
        self.config = config;
        self
    }

    /// The schema being indexed against.
    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The analyzer used for text fields.
    #[must_use]
    pub fn analyzer(&self) -> &Analyzer {
        &self.analyzer
    }

    /// Documents buffered so far.
    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.index.doc_count()
    }

    /// Whether nothing has been buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Whether a threshold has been crossed.
    #[must_use]
    pub fn should_flush(&self) -> bool {
        self.index.doc_count() >= self.config.max_buffered_docs
            || self.estimated_bytes() >= self.config.max_buffered_bytes
    }

    /// Rough memory held by the buffer.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        self.index.estimated_bytes() + self.store.estimated_bytes()
    }

    /// Indexes a document, returning its id.
    pub fn add(&mut self, doc: &Document) -> Result<DocId> {
        let id = self.index.allocate_doc();

        for (field_id, values) in doc.iter() {
            let field = self.schema.field_by_id(field_id).ok_or_else(|| {
                Error::schema(format!("field id {field_id} is not in the schema"))
            })?;

            for value in values {
                if !value.matches(field.ty) {
                    return Err(Error::schema(format!(
                        "field {} is {} but was given a different value type",
                        field.name,
                        field.ty.as_str()
                    )));
                }
            }

            if !field.options.indexed {
                continue;
            }

            let mut length = 0u32;

            for value in values {
                match field.ty {
                    FieldType::Text => {
                        let text = value.as_text();
                        for token in self.analyzer.analyze(&text) {
                            let position = field.options.positions.then_some(token.position);
                            self.index.add_term(field_id, &token.text, id, position);
                            length += 1;
                        }
                    }
                    FieldType::Keyword => {
                        let text = value.as_text();
                        for token in self.keyword_analyzer.analyze(&text) {
                            self.index.add_term(field_id, &token.text, id, None);
                            length += 1;
                        }
                    }
                    FieldType::Numeric => {
                        // Indexed as its canonical decimal form, so an exact
                        // match works. Range queries read the stored value
                        // instead, which is why numeric fields must be stored.
                        let text = value.as_text();
                        self.index.add_term(field_id, &text, id, None);
                        length += 1;
                    }
                }
            }

            if length > 0 {
                self.index.record_field_length(id, field_id, length);
            }
        }

        self.store.put(id, doc, &self.schema);
        Ok(id)
    }

    /// Parses JSON into a document and indexes it.
    pub fn add_json(&mut self, json: &Json) -> Result<DocId> {
        let doc = Document::from_json(&self.schema, json)?;
        self.add(&doc)
    }

    /// Parses a JSON string and indexes it.
    pub fn add_json_str(&mut self, raw: &str) -> Result<DocId> {
        let json = crate::json::parse(raw)?;
        self.add_json(&json)
    }

    /// Indexes every document in a JSON array.
    ///
    /// Stops at the first rejected document and reports its index, so a caller
    /// can fix that one rather than guessing which of a thousand failed.
    pub fn add_json_array(&mut self, json: &Json) -> Result<Vec<DocId>> {
        let items = json
            .as_array()
            .ok_or_else(|| Error::json("expected an array of documents"))?;

        let mut ids = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let id = self
                .add_json(item)
                .map_err(|e| Error::schema(format!("document {i}: {e}")))?;
            ids.push(id);
        }

        Ok(ids)
    }

    /// Convenience for a single-text-field document.
    pub fn add_text(&mut self, field: &str, text: &str) -> Result<DocId> {
        let field_id = self.schema.field_id(field)?;
        let mut doc = Document::new();
        doc.add(field_id, Value::Text(text.to_string()));
        self.add(&doc)
    }

    /// Seals the buffer into a segment and resets the writer.
    ///
    /// Returns `None` when nothing was buffered, so a caller can flush
    /// unconditionally in a loop without producing empty segments.
    pub fn flush(&mut self) -> Option<Segment> {
        if self.index.is_empty() {
            return None;
        }

        let index = std::mem::take(&mut self.index);
        let store = std::mem::take(&mut self.store);

        Some(Segment::new(index, store, &self.schema))
    }

    /// Discards everything buffered.
    pub fn clear(&mut self) {
        self.index = InvertedIndex::new();
        self.store = DocStore::new();
    }

    /// Borrows the buffer for inspection, without flushing.
    #[must_use]
    pub fn index(&self) -> &InvertedIndex {
        &self.index
    }

    /// Borrows the stored fields, without flushing.
    #[must_use]
    pub fn store(&self) -> &DocStore {
        &self.store
    }
}

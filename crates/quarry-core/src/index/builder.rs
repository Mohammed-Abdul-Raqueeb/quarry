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

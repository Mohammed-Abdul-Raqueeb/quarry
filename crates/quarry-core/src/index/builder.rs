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

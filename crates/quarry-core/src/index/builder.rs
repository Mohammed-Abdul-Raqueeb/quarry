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

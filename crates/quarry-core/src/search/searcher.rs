//! Running queries against segments.

use super::collector::{ScoredDoc, TopK};
use super::highlight::{self, HighlightOptions};
use crate::doc::{DocId, Document};
use crate::error::{Error, Result};
use crate::index::posting::PostingList;
use crate::index::segment::Segment;
use crate::schema::{FieldId, Schema};
use crate::score::bm25::Bm25;
use crate::score::Scorer;
use crate::tokenize::Analyzer;
use std::collections::HashMap;

/// A document a query matched.
#[derive(Debug, Clone)]
pub struct Hit {
    /// Index-wide document id.
    pub doc: DocId,
    /// Relevance score.
    pub score: f32,
    /// Which segment it came from, and its id within that segment.
    pub segment: usize,
    /// The id inside its segment, for looking up stored fields.
    pub local_doc: DocId,
}

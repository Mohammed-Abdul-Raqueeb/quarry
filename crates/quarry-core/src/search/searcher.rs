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

/// A query's results.
#[derive(Debug, Clone, Default)]
pub struct Results {
    /// Matches, best first.
    pub hits: Vec<Hit>,
    /// Documents matched, including those beyond the requested limit.
    pub total_hits: u64,
    /// How long the query took.
    pub elapsed: std::time::Duration,
}

impl Results {
    /// Whether nothing matched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// How many matches were returned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// The best score, if anything matched.
    #[must_use]
    pub fn max_score(&self) -> Option<f32> {
        self.hits.first().map(|h| h.score)
    }
}

/// A term to look for, already analyzed.
#[derive(Debug, Clone, PartialEq)]
pub struct TermQuery {
    /// Which field.
    pub field: FieldId,
    /// The analyzed term.
    pub term: String,
    /// Multiplier on this term's contribution.
    pub boost: f32,
}

impl TermQuery {
    /// A term query with no boost.
    #[must_use]
    pub fn new(field: FieldId, term: impl Into<String>) -> Self {
        Self {
            field,
            term: term.into(),
            boost: 1.0,
        }
    }

    /// The same query with a boost applied.
    #[must_use]
    pub fn boosted(mut self, boost: f32) -> Self {
        self.boost = boost;
        self
    }
}

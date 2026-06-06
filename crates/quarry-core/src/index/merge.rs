//! Folding segments together.
//!
//! Every flush produces a segment, so a long-lived index accumulates them, and
//! a query has to visit each one. Merging trades write work for read speed:
//! fewer segments means fewer dictionary lookups per term and a smaller
//! per-query constant.
//!
//! The important detail is document id remapping. Ids are dense and
//! segment-local, so segment B's document 0 is not segment A's document 0.
//! Merging assigns a fresh id space by offsetting each segment's ids by the
//! count of everything before it, which keeps them dense and ascending — and
//! therefore keeps the gap encoding small.

use super::inverted::{FieldStats, InvertedIndex, TermKey};
use super::posting::{Posting, PostingList};
use super::segment::Segment;
use crate::error::{Error, Result};
use crate::schema::{FieldId, Schema};
use crate::store::docstore::DocStore;
use std::collections::HashMap;

/// When merging is worth doing.
#[derive(Debug, Clone, Copy)]
pub struct MergePolicy {
    /// Merge once there are at least this many segments.
    pub min_segments: usize,
    /// Never merge more than this many at once.
    pub max_segments_per_merge: usize,
    /// Leave a segment alone once it is this large, in documents.
    ///
    /// Rewriting a large segment to absorb a small one copies the large one for
    /// almost no read benefit, so past this size a segment stops being a
    /// candidate.
    pub max_merged_docs: u32,
}

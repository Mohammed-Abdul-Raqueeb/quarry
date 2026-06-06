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

impl Default for MergePolicy {
    fn default() -> Self {
        Self {
            min_segments: 4,
            max_segments_per_merge: 10,
            max_merged_docs: 5_000_000,
        }
    }
}

impl MergePolicy {
    /// Which segments to merge next, as indices into `segments`.
    ///
    /// Picks the smallest candidates first. Merging small segments together is
    /// where almost all of the benefit is: it removes the most segments from
    /// the read path per byte rewritten.
    #[must_use]
    pub fn select(&self, segments: &[Segment]) -> Vec<usize> {
        if segments.len() < self.min_segments {
            return Vec::new();
        }

        let mut candidates: Vec<(usize, u32)> = segments
            .iter()
            .enumerate()
            .filter(|(_, s)| s.doc_count() < self.max_merged_docs)
            .map(|(i, s)| (i, s.doc_count()))
            .collect();

        if candidates.len() < 2 {
            return Vec::new();
        }

        candidates.sort_by_key(|(_, count)| *count);
        candidates.truncate(self.max_segments_per_merge);

        let mut selected: Vec<usize> = candidates.into_iter().map(|(i, _)| i).collect();
        selected.sort_unstable();
        selected
    }
}

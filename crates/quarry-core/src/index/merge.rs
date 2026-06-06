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

/// Merges segments into one.
///
/// Every input must have been written against `schema`; a mismatch is an error
/// rather than a best-effort merge, because field ids are positional and
/// silently mixing two schemas would attribute terms to the wrong fields.
pub fn merge(segments: &[Segment], schema: &Schema) -> Result<Segment> {
    if segments.is_empty() {
        return Err(Error::corrupt("nothing to merge"));
    }

    for segment in segments {
        segment.check_schema(schema)?;
    }

    if segments.len() == 1 {
        return Ok(segments[0].clone());
    }

    let total_docs: u32 = segments
        .iter()
        .map(Segment::doc_count)
        .try_fold(0u32, u32::checked_add)
        .ok_or_else(|| Error::limit("merged segment would exceed u32 document ids"))?;

    // Each segment's ids shift up by everything before it.
    let mut offsets = Vec::with_capacity(segments.len());
    let mut running = 0u32;
    for segment in segments {
        offsets.push(running);
        running += segment.doc_count();
    }

    let mut index = InvertedIndex::new();
    index.set_doc_count(total_docs);

    let mut store = DocStore::new();
    let mut stats: HashMap<FieldId, FieldStats> = HashMap::new();
    let mut merged_terms: HashMap<TermKey, PostingList> = HashMap::new();

    for (segment, offset) in segments.iter().zip(offsets.iter().copied()) {
        for (key, list) in segment.index.iter_terms() {
            let entry = merged_terms
                .entry(key.clone())
                .or_insert_with(|| PostingList::new(key.field));

            for posting in &list.postings {
                entry.postings.push(Posting {
                    doc: posting.doc + offset,
                    term_freq: posting.term_freq,
                    positions: posting.positions.clone(),
                });
            }
        }

        for field in segment.index.active_fields() {
            let segment_stats = segment.index.field_stats(field);
            let entry = stats.entry(field).or_default();
            entry.doc_count += segment_stats.doc_count;
            entry.total_length += segment_stats.total_length;

            for doc in 0..segment.doc_count() {
                let length = segment.index.field_length(doc, field);
                if length > 0 {
                    // Set directly rather than via `record_field_length`, which
                    // would accumulate statistics that are already summed above.
                    index.set_field_length(doc + offset, field, length);
                }
            }
        }

        for doc in segment.store.doc_ids() {
            if let Some(values) = segment.store.get(doc) {
                store.put_raw(doc + offset, values.to_vec());
            }
        }
    }

    // Postings were appended segment by segment, and segments are visited in
    // order, so each list is already ascending. Sorting anyway costs little and
    // means a caller passing segments out of order still gets a valid index
    // rather than a subtly broken one.
    for list in merged_terms.values_mut() {
        list.postings.sort_unstable_by_key(|p| p.doc);
    }

    for (key, list) in merged_terms {
        index.insert_list(key, list);
    }

    index.set_field_stats(stats);

    Ok(Segment::new(index, store, schema))
}

/// Merges, then reports how much the merge saved.
pub fn merge_with_report(segments: &[Segment], schema: &Schema) -> Result<(Segment, MergeReport)> {
    let before: usize = segments.iter().map(|s| s.write().len()).sum();
    let before_terms: usize = segments.iter().map(|s| s.index.term_count()).sum();

    let merged = merge(segments, schema)?;
    let after = merged.write().len();

    let report = MergeReport {
        segments_in: segments.len(),
        docs: merged.doc_count(),
        terms_before: before_terms,
        terms_after: merged.index.term_count(),
        bytes_before: before,
        bytes_after: after,
    };

    Ok((merged, report))
}

/// What a merge accomplished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeReport {
    /// Segments merged.
    pub segments_in: usize,
    /// Documents in the result.
    pub docs: u32,
    /// Term entries across the inputs, counting duplicates.
    pub terms_before: usize,
    /// Distinct term entries in the result.
    pub terms_after: usize,
    /// Total input size in bytes.
    pub bytes_before: usize,
    /// Output size in bytes.
    pub bytes_after: usize,
}

impl MergeReport {
    /// Fraction of bytes saved, or 0.0 if the merge grew the index.
    #[must_use]
    pub fn bytes_saved_ratio(&self) -> f64 {
        if self.bytes_before == 0 || self.bytes_after >= self.bytes_before {
            return 0.0;
        }
        1.0 - (self.bytes_after as f64 / self.bytes_before as f64)
    }

    /// Term entries eliminated by combining duplicates across segments.
    #[must_use]
    pub fn terms_collapsed(&self) -> usize {
        self.terms_before.saturating_sub(self.terms_after)
    }
}

//! Index statistics, for the `stats` command and the server's status endpoint.

use crate::index::Segment;
use crate::schema::Schema;

/// A summary of what an index contains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexStats {
    /// How many segments make up the index.
    pub segments: usize,
    /// Documents across every segment.
    pub docs: u32,
    /// Distinct field/term pairs, summed across segments.
    ///
    /// Counts duplicates: the same term in two segments counts twice, because
    /// that is what a query actually has to look up.
    pub terms: usize,
    /// Total serialized size in bytes.
    pub bytes: usize,
    /// Fields defined in the schema.
    pub fields: usize,
}

impl IndexStats {
    /// Gathers statistics from a set of segments.
    #[must_use]
    pub fn collect(segments: &[Segment], schema: &Schema) -> Self {
        Self {
            segments: segments.len(),
            docs: segments.iter().map(Segment::doc_count).sum(),
            terms: segments.iter().map(|s| s.index.term_count()).sum(),
            bytes: segments.iter().map(|s| s.write().len()).sum(),
            fields: schema.len(),
        }
    }

    /// Mean bytes per document, or 0.0 for an empty index.
    #[must_use]
    pub fn bytes_per_doc(&self) -> f64 {
        if self.docs == 0 {
            0.0
        } else {
            self.bytes as f64 / f64::from(self.docs)
        }
    }

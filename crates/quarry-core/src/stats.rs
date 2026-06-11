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

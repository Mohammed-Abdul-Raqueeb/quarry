//! The index: dictionary, postings, segments and merging.

pub mod builder;
pub mod inverted;
pub mod merge;
pub mod posting;
pub mod segment;

pub use builder::{IndexWriter, WriterConfig};
pub use inverted::{FieldStats, InvertedIndex, TermKey};
pub use merge::{merge, MergePolicy, MergeReport};
pub use posting::{Posting, PostingList};
pub use segment::{Segment, SegmentMeta, FORMAT_VERSION};

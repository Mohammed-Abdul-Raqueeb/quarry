//! Posting lists and the in-memory inverted index.

// Exact float comparison is deliberate here: the values are constants from
// the source or small sums chosen to be exactly representable, so a tolerance
// would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::index::inverted::TermKey;
use quarry_core::index::{InvertedIndex, Posting, PostingList};
use quarry_core::store::blob::{BlobReader, BlobWriter};

fn list_with(docs: &[(u32, u32)]) -> PostingList {
    let mut list = PostingList::new(0);
    for (doc, freq) in docs {
        list.postings.push(Posting::new(*doc, *freq));
    }
    list
}

// ---- Posting ------------------------------------------------------------

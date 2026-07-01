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

#[test]
fn a_posting_without_positions_reports_none() {
    let posting = Posting::new(3, 2);
    assert!(!posting.has_positions());
}

#[test]
fn a_posting_with_positions_takes_its_frequency_from_them() {
    let posting = Posting::with_positions(3, vec![1, 5, 9]);
    assert_eq!(posting.term_freq, 3);
    assert!(posting.has_positions());
}

#[test]
fn a_posting_with_no_positions_has_zero_frequency() {
    assert_eq!(Posting::with_positions(1, vec![]).term_freq, 0);
}

// ---- PostingList --------------------------------------------------------

#[test]
fn a_new_list_is_empty() {
    let list = PostingList::new(0);
    assert!(list.is_empty());
    assert_eq!(list.doc_freq(), 0);
}

#[test]
fn doc_freq_counts_documents_not_occurrences() {
    // This is the document frequency BM25's IDF is computed from, so counting
    // occurrences here would weight common words wrongly.
    let list = list_with(&[(1, 10), (2, 20)]);
    assert_eq!(list.doc_freq(), 2);
}

#[test]
fn total_term_freq_sums_occurrences() {
    let list = list_with(&[(1, 10), (2, 20)]);
    assert_eq!(list.total_term_freq(), 30);
}

#[test]
fn push_accepts_ascending_document_ids() {
    let mut list = PostingList::new(0);
    assert!(list.push(Posting::new(1, 1)).is_ok());
    assert!(list.push(Posting::new(5, 1)).is_ok());
}

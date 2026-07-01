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

#[test]
fn push_rejects_a_descending_document_id() {
    // The ordering is load-bearing: intersection and gap encoding both rely on
    // it, so a violation is an error rather than something to sort out later.
    let mut list = PostingList::new(0);
    list.push(Posting::new(5, 1)).expect("first push");
    assert!(list.push(Posting::new(2, 1)).is_err());
}

#[test]
fn push_rejects_a_duplicate_document_id() {
    let mut list = PostingList::new(0);
    list.push(Posting::new(5, 1)).expect("first push");
    assert!(list.push(Posting::new(5, 1)).is_err());
}

#[test]
fn get_finds_a_posting_by_document() {
    let list = list_with(&[(1, 1), (7, 3), (9, 1)]);
    assert_eq!(list.get(7).map(|p| p.term_freq), Some(3));
}

#[test]
fn get_returns_none_for_an_absent_document() {
    let list = list_with(&[(1, 1), (9, 1)]);
    assert!(list.get(5).is_none());
}

#[test]
fn contains_reports_membership() {
    let list = list_with(&[(1, 1), (9, 1)]);
    assert!(list.contains(9));
    assert!(!list.contains(2));
}

#[test]
fn doc_ids_returns_them_in_order() {
    let list = list_with(&[(1, 1), (4, 1), (9, 1)]);
    assert_eq!(list.doc_ids(), vec![1, 4, 9]);
}

// ---- set operations -----------------------------------------------------

#[test]
fn intersect_keeps_only_shared_documents() {
    let left = list_with(&[(1, 1), (2, 1), (5, 1)]);
    let right = list_with(&[(2, 1), (5, 1), (8, 1)]);

    assert_eq!(left.intersect(&right), vec![2, 5]);
}

#[test]
fn intersect_of_disjoint_lists_is_empty() {
    let left = list_with(&[(1, 1), (3, 1)]);
    let right = list_with(&[(2, 1), (4, 1)]);

    assert!(left.intersect(&right).is_empty());
}

#[test]
fn intersect_with_an_empty_list_is_empty() {
    let left = list_with(&[(1, 1)]);
    assert!(left.intersect(&PostingList::new(0)).is_empty());
}

#[test]
fn intersect_is_commutative() {
    let left = list_with(&[(1, 1), (2, 1), (5, 1)]);
    let right = list_with(&[(2, 1), (5, 1), (8, 1)]);

    assert_eq!(left.intersect(&right), right.intersect(&left));
}

#[test]
fn union_merges_without_duplicates() {
    let left = list_with(&[(1, 1), (3, 1)]);
    let right = list_with(&[(3, 1), (5, 1)]);

    assert_eq!(left.union(&right), vec![1, 3, 5]);
}

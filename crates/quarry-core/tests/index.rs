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

#[test]
fn union_with_an_empty_list_returns_the_other() {
    let left = list_with(&[(1, 1), (3, 1)]);
    assert_eq!(left.union(&PostingList::new(0)), vec![1, 3]);
}

#[test]
fn union_keeps_the_result_sorted() {
    let left = list_with(&[(1, 1), (9, 1)]);
    let right = list_with(&[(4, 1), (6, 1)]);

    let merged = left.union(&right);
    assert!(merged.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn difference_removes_shared_documents() {
    let left = list_with(&[(1, 1), (2, 1), (5, 1)]);
    let right = list_with(&[(2, 1)]);

    assert_eq!(left.difference(&right), vec![1, 5]);
}

#[test]
fn difference_with_an_empty_list_returns_everything() {
    let left = list_with(&[(1, 1), (2, 1)]);
    assert_eq!(left.difference(&PostingList::new(0)), vec![1, 2]);
}

#[test]
fn difference_of_identical_lists_is_empty() {
    let left = list_with(&[(1, 1), (2, 1)]);
    assert!(left.difference(&left).is_empty());
}

// ---- posting list encoding ---------------------------------------------

fn round_trip(list: &PostingList) -> PostingList {
    let mut writer = BlobWriter::new();
    list.encode(&mut writer);
    let bytes = writer.finish();

    PostingList::decode(&mut BlobReader::new(&bytes)).expect("should decode")
}

#[test]
fn an_empty_list_round_trips() {
    let list = PostingList::new(3);
    let decoded = round_trip(&list);

    assert_eq!(decoded.field, 3);
    assert!(decoded.is_empty());
}

#[test]
fn a_list_without_positions_round_trips() {
    let list = list_with(&[(1, 2), (7, 1), (100, 5)]);
    assert_eq!(round_trip(&list), list);
}

#[test]
fn a_list_with_positions_round_trips() {
    let mut list = PostingList::new(1);
    list.postings
        .push(Posting::with_positions(0, vec![0, 3, 7]));
    list.postings.push(Posting::with_positions(4, vec![1]));

    assert_eq!(round_trip(&list), list);
}

#[test]
fn a_list_starting_at_document_zero_round_trips() {
    let list = list_with(&[(0, 1), (1, 1)]);
    assert_eq!(round_trip(&list), list);
}

#[test]
fn a_list_with_large_document_ids_round_trips() {
    let list = list_with(&[(1_000_000, 1), (2_000_000, 1)]);
    assert_eq!(round_trip(&list), list);
}

#[test]
fn a_long_list_round_trips() {
    let docs: Vec<(u32, u32)> = (0..500).map(|i| (i * 3, 1 + i % 7)).collect();
    let list = list_with(&docs);

    assert_eq!(round_trip(&list), list);
}

#[test]
fn gap_encoding_keeps_a_dense_list_small() {
    let docs: Vec<(u32, u32)> = (10_000..10_300).map(|i| (i, 1)).collect();
    let list = list_with(&docs);

    let mut writer = BlobWriter::new();
    list.encode(&mut writer);

    // 300 postings with adjacent ids and a frequency of 1 should cost roughly
    // two bytes each, not the eight a fixed layout would.
    assert!(
        writer.len() < docs.len() * 4,
        "300 dense postings took {} bytes",
        writer.len()
    );
}

#[test]
fn decoding_a_truncated_list_fails() {
    let list = list_with(&[(1, 1), (2, 1), (3, 1)]);
    let mut writer = BlobWriter::new();
    list.encode(&mut writer);
    let bytes = writer.finish();

    let cut = &bytes[..bytes.len() / 2];
    assert!(PostingList::decode(&mut BlobReader::new(cut)).is_err());
}

#[test]
fn decoding_an_empty_buffer_fails() {
    assert!(PostingList::decode(&mut BlobReader::new(&[])).is_err());
}

// ---- InvertedIndex ------------------------------------------------------

#[test]
fn a_new_index_is_empty() {
    let index = InvertedIndex::new();
    assert!(index.is_empty());
    assert_eq!(index.doc_count(), 0);
    assert_eq!(index.term_count(), 0);
}

#[test]
fn allocate_doc_assigns_dense_ascending_ids() {
    // Dense ids are what allow the gap encoding in the posting codec.
    let mut index = InvertedIndex::new();
    assert_eq!(index.allocate_doc(), 0);
    assert_eq!(index.allocate_doc(), 1);
    assert_eq!(index.doc_count(), 2);
}

#[test]
fn add_term_creates_a_posting_list() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "rust", doc, None);

    assert_eq!(index.doc_freq(0, "rust"), 1);
}

#[test]
fn repeating_a_term_in_one_document_raises_its_frequency() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    for _ in 0..3 {
        index.add_term(0, "rust", doc, None);
    }

    let list = index.postings(0, "rust").expect("list exists");
    assert_eq!(list.doc_freq(), 1);
    assert_eq!(list.total_term_freq(), 3);
}

#[test]
fn the_same_term_in_two_documents_gives_a_frequency_of_two() {
    let mut index = InvertedIndex::new();
    for _ in 0..2 {
        let doc = index.allocate_doc();
        index.add_term(0, "rust", doc, None);
    }

    assert_eq!(index.doc_freq(0, "rust"), 2);
}

#[test]
fn positions_accumulate_in_order() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    for position in [0u32, 4, 9] {
        index.add_term(0, "rust", doc, Some(position));
    }

    let list = index.postings(0, "rust").expect("list exists");
    assert_eq!(
        list.get(doc).map(|p| p.positions.clone()),
        Some(vec![0, 4, 9])
    );
}

#[test]
fn the_same_term_in_two_fields_keeps_separate_lists() {
    // Sharing one list and filtering by field would make the document
    // frequency wrong for every field-scoped query.
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "rust", doc, None);
    index.add_term(1, "rust", doc, None);

    assert_eq!(index.term_count(), 2);
    assert_eq!(index.doc_freq(0, "rust"), 1);
    assert_eq!(index.doc_freq(1, "rust"), 1);
}

#[test]
fn doc_freq_is_zero_for_an_unknown_term() {
    assert_eq!(InvertedIndex::new().doc_freq(0, "absent"), 0);
}

#[test]
fn postings_returns_none_for_an_unknown_term() {
    assert!(InvertedIndex::new().postings(0, "absent").is_none());
}

#[test]
fn records_and_reports_a_field_length() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.record_field_length(doc, 0, 12);

    assert_eq!(index.field_length(doc, 0), 12);
}

#[test]
fn field_length_is_zero_when_unrecorded() {
    assert_eq!(InvertedIndex::new().field_length(0, 0), 0);
}

#[test]
fn field_stats_accumulate_across_documents() {
    let mut index = InvertedIndex::new();
    for length in [10u32, 20, 30] {
        let doc = index.allocate_doc();
        index.record_field_length(doc, 0, length);
    }

    let stats = index.field_stats(0);
    assert_eq!(stats.doc_count, 3);
    assert_eq!(stats.total_length, 60);
    assert_eq!(stats.average_length(), 20.0);
}

#[test]
fn average_length_is_one_for_an_unseen_field() {
    // Returning 1.0 rather than 0.0 keeps BM25 from dividing by zero.
    assert_eq!(InvertedIndex::new().field_stats(0).average_length(), 1.0);
}

#[test]
fn terms_are_iterated_in_sorted_order() {
    // The segment writer front-codes the dictionary, which only works if the
    // terms arrive sorted.
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    for term in ["zebra", "apple", "mango"] {
        index.add_term(0, term, doc, None);
    }

    let terms: Vec<&str> = index
        .iter_terms()
        .map(|(key, _)| key.text.as_str())
        .collect();
    assert_eq!(terms, ["apple", "mango", "zebra"]);
}

#[test]
fn terms_with_prefix_finds_a_contiguous_range() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    for term in ["borrow", "borrowed", "borrowing", "break"] {
        index.add_term(0, term, doc, None);
    }

    assert_eq!(
        index.terms_with_prefix(0, "borrow"),
        ["borrow", "borrowed", "borrowing"]
    );
}

#[test]
fn terms_with_prefix_returns_nothing_for_an_unmatched_prefix() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "rust", doc, None);

    assert!(index.terms_with_prefix(0, "python").is_empty());
}

#[test]
fn terms_with_prefix_is_scoped_to_one_field() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "rust", doc, None);
    index.add_term(1, "rust", doc, None);

    assert_eq!(index.terms_with_prefix(0, "rus").len(), 1);
}

#[test]
fn an_empty_prefix_returns_every_term_in_the_field() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "a", doc, None);
    index.add_term(0, "b", doc, None);

    assert_eq!(index.terms_with_prefix(0, "").len(), 2);
}

#[test]
fn terms_in_field_lists_only_that_field() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "one", doc, None);
    index.add_term(1, "two", doc, None);

    assert_eq!(index.terms_in_field(0), ["one"]);
    assert_eq!(index.terms_in_field(1), ["two"]);
}

#[test]
fn most_frequent_terms_orders_by_document_frequency() {
    let mut index = InvertedIndex::new();
    for i in 0..3 {
        let doc = index.allocate_doc();
        index.add_term(0, "common", doc, None);
        if i == 0 {
            index.add_term(0, "rare", doc, None);
        }
    }

    assert_eq!(
        index.most_frequent_terms(0, 2),
        vec![("common", 3), ("rare", 1)]
    );
}

#[test]
fn most_frequent_terms_breaks_ties_alphabetically() {
    // Deterministic output matters for a CLI that prints this.
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    index.add_term(0, "zebra", doc, None);
    index.add_term(0, "apple", doc, None);

    assert_eq!(index.most_frequent_terms(0, 2)[0].0, "apple");
}

#[test]
fn most_frequent_terms_respects_its_limit() {
    let mut index = InvertedIndex::new();
    let doc = index.allocate_doc();
    for term in ["a", "b", "c", "d"] {
        index.add_term(0, term, doc, None);
    }

    assert_eq!(index.most_frequent_terms(0, 2).len(), 2);
}

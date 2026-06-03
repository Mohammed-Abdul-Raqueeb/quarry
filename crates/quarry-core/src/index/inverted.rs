//! The in-memory inverted index.

use super::posting::{Posting, PostingList};
use crate::doc::DocId;
use crate::schema::FieldId;
use std::collections::{BTreeMap, HashMap};

/// A term as the index keys it: a field plus the analyzed text.
///
/// The field is part of the key rather than a filter applied afterwards, so
/// `title:rust` and `body:rust` are separate lists. Sharing one list and
/// filtering by field would make the document frequency wrong for every
/// field-scoped query.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermKey {
    /// Which field.
    pub field: FieldId,
    /// The analyzed term.
    pub text: String,
}

impl TermKey {
    /// A key for a field and term.
    #[must_use]
    pub fn new(field: FieldId, text: impl Into<String>) -> Self {
        Self {
            field,
            text: text.into(),
        }
    }
}

/// Per-field length bookkeeping, needed by BM25.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldStats {
    /// How many documents have any value for the field.
    pub doc_count: u64,
    /// Total terms across every document, counting repeats.
    pub total_length: u64,
}

impl FieldStats {
    /// Mean field length, which BM25 normalises against.
    ///
    /// Returns 1.0 rather than 0.0 for an empty field so the caller never
    /// divides by zero; with no documents the normaliser is unused anyway.
    #[must_use]
    pub fn average_length(&self) -> f64 {
        if self.doc_count == 0 {
            1.0
        } else {
            self.total_length as f64 / self.doc_count as f64
        }
    }
}

/// An inverted index held in memory.
///
/// Terms live in a [`BTreeMap`] rather than a `HashMap` so iteration is in
/// sorted order. That ordering is what the segment writer relies on to
/// front-code the dictionary and what makes a prefix scan a contiguous range
/// instead of a full walk.
#[derive(Debug, Clone, Default)]
pub struct InvertedIndex {
    terms: BTreeMap<TermKey, PostingList>,
    doc_count: u32,
    field_stats: HashMap<FieldId, FieldStats>,
    /// Length of each field of each document, for BM25's length normalisation.
    doc_lengths: HashMap<(DocId, FieldId), u32>,
}

impl InvertedIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many documents have been added.
    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.doc_count
    }

    /// How many distinct field/term pairs exist.
    #[must_use]
    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    /// Whether nothing has been indexed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.doc_count == 0
    }

    /// Records that a document exists, returning its id.
    ///
    /// Ids are assigned densely in insertion order, which is what allows the
    /// gap encoding in the posting codec.
    pub fn allocate_doc(&mut self) -> DocId {
        let id = self.doc_count;
        self.doc_count += 1;
        id
    }

    /// Adds one occurrence of a term.
    ///
    /// Takes the position as an `Option` so a field indexed without positions
    /// does not pay for an empty vector per posting.
    pub fn add_term(&mut self, field: FieldId, text: &str, doc: DocId, position: Option<u32>) {
        let key = TermKey::new(field, text);
        let list = self
            .terms
            .entry(key)
            .or_insert_with(|| PostingList::new(field));

        // Documents arrive in id order, so an existing posting for this doc is
        // always the last one — no search needed.
        match list.postings.last_mut() {
            Some(last) if last.doc == doc => {
                last.term_freq += 1;
                if let Some(pos) = position {
                    last.positions.push(pos);
                }
            }
            _ => {
                let mut posting = Posting::new(doc, 1);
                if let Some(pos) = position {
                    posting.positions.push(pos);
                }
                list.postings.push(posting);
            }
        }
    }

    /// Records the analyzed length of one field of one document.
    pub fn record_field_length(&mut self, doc: DocId, field: FieldId, length: u32) {
        self.doc_lengths.insert((doc, field), length);
        let stats = self.field_stats.entry(field).or_default();
        stats.doc_count += 1;
        stats.total_length += u64::from(length);
    }

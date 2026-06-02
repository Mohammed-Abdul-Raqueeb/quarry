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

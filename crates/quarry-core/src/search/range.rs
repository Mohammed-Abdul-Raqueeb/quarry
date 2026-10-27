//! Numeric range matching.
//!
//! A numeric field is indexed as the canonical decimal form of its value, so
//! the dictionary for that field *is* the sorted set of values it holds. A
//! range is therefore answered by scanning that field's terms, parsing each
//! one, and unioning the postings of those that fall inside the bounds.
//!
//! The cost is proportional to the number of **distinct values** in the field,
//! not to the corpus size. For a year or a rating that is a handful of terms
//! and effectively free. For a high-cardinality field — a float price with a
//! million distinct values — it is a full scan of that field's dictionary, and
//! `docs/query-language.md` says so rather than leaving it to be discovered.
//!
//! A numeric trie, or range-encoded terms at several precisions, would make
//! that case logarithmic. Neither is built: the common case is low cardinality,
//! and the scan is a dozen lines against a few hundred.

use crate::doc::DocId;
use crate::index::Segment;
use crate::schema::FieldId;

/// A half-open or closed interval on a numeric field.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeQuery {
    /// Which field.
    pub field: FieldId,
    /// Lower bound, or `None` for unbounded below.
    pub lower: Option<f64>,
    /// Upper bound, or `None` for unbounded above.
    pub upper: Option<f64>,
    /// Whether the lower bound itself matches.
    pub include_lower: bool,
    /// Whether the upper bound itself matches.
    pub include_upper: bool,
}

impl RangeQuery {
    /// A closed interval, both ends inclusive.
    #[must_use]
    pub fn inclusive(field: FieldId, lower: f64, upper: f64) -> Self {
        Self {
            field,
            lower: Some(lower),
            upper: Some(upper),
            include_lower: true,
            include_upper: true,
        }
    }

    /// An open interval, neither end matching.
    #[must_use]
    pub fn exclusive(field: FieldId, lower: f64, upper: f64) -> Self {
        Self {
            field,
            lower: Some(lower),
            upper: Some(upper),
            include_lower: false,
            include_upper: false,
        }
    }

    /// Everything at or above `lower`.
    #[must_use]
    pub fn at_least(field: FieldId, lower: f64) -> Self {
        Self {
            field,
            lower: Some(lower),
            upper: None,
            include_lower: true,
            include_upper: true,
        }
    }

    /// Everything at or below `upper`.
    #[must_use]
    pub fn at_most(field: FieldId, upper: f64) -> Self {
        Self {
            field,
            lower: None,
            upper: Some(upper),
            include_lower: true,
            include_upper: true,
        }
    }

    /// Whether `value` falls inside the range.
    ///
    /// A NaN value never matches: it compares false against every bound, and
    /// silently treating it as a match would make a corrupt document appear in
    /// every range.
    #[must_use]
    pub fn contains(&self, value: f64) -> bool {
        if value.is_nan() {
            return false;
        }

        let above_lower = match self.lower {
            None => true,
            Some(lower) if self.include_lower => value >= lower,
            Some(lower) => value > lower,
        };

        let below_upper = match self.upper {
            None => true,
            Some(upper) if self.include_upper => value <= upper,
            Some(upper) => value < upper,
        };

        above_lower && below_upper
    }

    /// Whether the range can match anything at all.
    ///
    /// Crossed bounds, or equal bounds with either end excluded, describe an
    /// empty interval. Detecting it here lets the executor skip the dictionary
    /// scan entirely.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let (Some(lower), Some(upper)) = (self.lower, self.upper) else {
            return false;
        };

        if lower > upper {
            return true;
        }

        lower == upper && !(self.include_lower && self.include_upper)
    }

    /// Whether the range places no constraint at all.
    #[must_use]
    pub fn is_unbounded(&self) -> bool {
        self.lower.is_none() && self.upper.is_none()
    }
}

/// Documents in `segment` whose value for the range's field falls inside it.
///
/// Returns an ascending list, so it composes with the other set operations the
/// executor uses.
#[must_use]
pub fn matching_docs(segment: &Segment, range: &RangeQuery) -> Vec<DocId> {
    if range.is_empty() {
        return Vec::new();
    }

    let mut matched: Vec<DocId> = Vec::new();

    for term in segment.index.terms_in_field(range.field) {
        // A term that is not a number is not an error: a field could have been
        // retyped between indexing runs, and skipping is better than refusing
        // to answer.
        let Ok(value) = term.parse::<f64>() else {
            continue;
        };

        if !range.contains(value) {
            continue;
        }

        if let Some(list) = segment.index.postings(range.field, term) {
            matched = merge_ascending(&matched, &list.doc_ids());
        }
    }

    matched
}

/// Counts documents in range without materialising the list.
///
/// A multi-valued field can put one document in several matching terms, so the
/// ids still have to be deduplicated; this exists to make the intent explicit
/// at the call site rather than to save the allocation.
#[must_use]
pub fn count_matching(segment: &Segment, range: &RangeQuery) -> usize {
    matching_docs(segment, range).len()
}

/// Unions two ascending slices, dropping duplicates.
///
/// Kept local rather than reaching for the searcher's copy, so this module has
/// no dependency on the query executor and can be tested on its own.
fn merge_ascending(a: &[DocId], b: &[DocId]) -> Vec<DocId> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);

    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                out.push(a[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(b[j]);
                j += 1;
            }
        }
    }

    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

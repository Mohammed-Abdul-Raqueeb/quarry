//! Running queries against segments.

use super::collector::{ScoredDoc, TopK};
use super::highlight::{self, HighlightOptions};
use crate::doc::{DocId, Document};
use crate::error::{Error, Result};
use crate::index::posting::PostingList;
use crate::index::segment::Segment;
use crate::schema::{FieldId, Schema};
use crate::score::bm25::Bm25;
use crate::score::Scorer;
use crate::tokenize::Analyzer;
use std::collections::HashMap;

/// A document a query matched.
#[derive(Debug, Clone)]
pub struct Hit {
    /// Index-wide document id.
    pub doc: DocId,
    /// Relevance score.
    pub score: f32,
    /// Which segment it came from, and its id within that segment.
    pub segment: usize,
    /// The id inside its segment, for looking up stored fields.
    pub local_doc: DocId,
}

/// A query's results.
#[derive(Debug, Clone, Default)]
pub struct Results {
    /// Matches, best first.
    pub hits: Vec<Hit>,
    /// Documents matched, including those beyond the requested limit.
    pub total_hits: u64,
    /// How long the query took.
    pub elapsed: std::time::Duration,
}

impl Results {
    /// Whether nothing matched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// How many matches were returned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// The best score, if anything matched.
    #[must_use]
    pub fn max_score(&self) -> Option<f32> {
        self.hits.first().map(|h| h.score)
    }
}

/// A term to look for, already analyzed.
#[derive(Debug, Clone, PartialEq)]
pub struct TermQuery {
    /// Which field.
    pub field: FieldId,
    /// The analyzed term.
    pub term: String,
    /// Multiplier on this term's contribution.
    pub boost: f32,
}

impl TermQuery {
    /// A term query with no boost.
    #[must_use]
    pub fn new(field: FieldId, term: impl Into<String>) -> Self {
        Self {
            field,
            term: term.into(),
            boost: 1.0,
        }
    }

    /// The same query with a boost applied.
    #[must_use]
    pub fn boosted(mut self, boost: f32) -> Self {
        self.boost = boost;
        self
    }
}

/// What a document must, may and must not match.
///
/// Modelled on the three-clause boolean rather than a nested expression tree,
/// because that is what the executor can run in one pass: `must` drives the
/// candidate set, `should` only contributes score, and `must_not` filters.
#[derive(Debug, Clone, Default)]
pub struct BooleanQuery {
    /// Every one of these must match.
    pub must: Vec<TermQuery>,
    /// Each group must have at least one matching term.
    ///
    /// This is what a bare term across several default fields means: the word
    /// has to appear in *one* of title or body, not in both. Without a group
    /// like this, expanding one term into several `must` clauses silently
    /// demands that every field contain it.
    pub must_any: Vec<Vec<TermQuery>>,
    /// Any of these may match, and matching raises the score.
    pub should: Vec<TermQuery>,
    /// None of these may match.
    pub must_not: Vec<TermQuery>,
    /// Phrases whose terms must appear adjacently, in order.
    pub phrases: Vec<PhraseQuery>,
    /// How many `should` clauses must match when there are no `must` clauses.
    pub minimum_should_match: usize,
}

impl BooleanQuery {
    /// An empty query, matching nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a required term.
    #[must_use]
    pub fn must(mut self, term: TermQuery) -> Self {
        self.must.push(term);
        self
    }

    /// Adds a group of which at least one term must match.
    #[must_use]
    pub fn must_any(mut self, terms: Vec<TermQuery>) -> Self {
        if !terms.is_empty() {
            self.must_any.push(terms);
        }
        self
    }

    /// Adds an optional term.
    #[must_use]
    pub fn should(mut self, term: TermQuery) -> Self {
        self.should.push(term);
        self
    }

    /// Adds a forbidden term.
    #[must_use]
    pub fn must_not(mut self, term: TermQuery) -> Self {
        self.must_not.push(term);
        self
    }

    /// Adds a required phrase.
    #[must_use]
    pub fn phrase(mut self, phrase: PhraseQuery) -> Self {
        self.phrases.push(phrase);
        self
    }

    /// Whether the query has no clauses at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.must.is_empty()
            && self.must_any.is_empty()
            && self.should.is_empty()
            && self.must_not.is_empty()
            && self.phrases.is_empty()
    }

    /// Every term mentioned, for highlighting.
    #[must_use]
    pub fn all_terms(&self) -> Vec<String> {
        let mut terms: Vec<String> = self
            .must
            .iter()
            .chain(&self.should)
            .chain(self.must_any.iter().flatten())
            .map(|t| t.term.clone())
            .collect();

        for phrase in &self.phrases {
            terms.extend(phrase.terms.iter().cloned());
        }

        terms.sort();
        terms.dedup();
        terms
    }
}

/// Terms that must appear adjacently and in order.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseQuery {
    /// Which field.
    pub field: FieldId,
    /// The analyzed terms, in order.
    pub terms: Vec<String>,
    /// Extra positions allowed between adjacent terms.
    ///
    /// Zero demands strict adjacency. A slop of 1 lets one unexpected token sit
    /// between them, which is what makes a phrase survive a dropped stopword.
    pub slop: u32,
}

impl PhraseQuery {
    /// A strict phrase.
    #[must_use]
    pub fn new(field: FieldId, terms: Vec<String>) -> Self {
        Self {
            field,
            terms,
            slop: 0,
        }
    }

    /// A phrase tolerating gaps.
    #[must_use]
    pub fn with_slop(mut self, slop: u32) -> Self {
        self.slop = slop;
        self
    }
}

/// Executes queries over a set of segments.
#[derive(Debug)]
pub struct Searcher<'a> {
    segments: &'a [Segment],
    schema: &'a Schema,
    scorer: Box<dyn Scorer>,
    /// Running document total, so hits carry an index-wide id.
    offsets: Vec<DocId>,
    total_docs: u64,
}

impl<'a> Searcher<'a> {
    /// A searcher over `segments`, scoring with BM25.
    #[must_use]
    pub fn new(segments: &'a [Segment], schema: &'a Schema) -> Self {
        Self::with_scorer(segments, schema, Box::new(Bm25::default()))
    }

    /// A searcher with an explicit scorer.
    #[must_use]
    pub fn with_scorer(
        segments: &'a [Segment],
        schema: &'a Schema,
        scorer: Box<dyn Scorer>,
    ) -> Self {
        let mut offsets = Vec::with_capacity(segments.len());
        let mut running = 0u32;
        for segment in segments {
            offsets.push(running);
            running += segment.doc_count();
        }

        Self {
            segments,
            schema,
            scorer,
            offsets,
            total_docs: u64::from(running),
        }
    }

    /// Documents across every segment.
    #[must_use]
    pub fn doc_count(&self) -> u64 {
        self.total_docs
    }

    /// The schema being searched.
    #[must_use]
    pub fn schema(&self) -> &Schema {
        self.schema
    }

    /// Runs a boolean query, returning the best `limit` matches.
    pub fn search(&self, query: &BooleanQuery, limit: usize) -> Result<Results> {
        let started = std::time::Instant::now();

        if query.is_empty() {
            return Ok(Results {
                elapsed: started.elapsed(),
                ..Results::default()
            });
        }

        let mut collector = TopK::new(limit);

        for (segment_index, segment) in self.segments.iter().enumerate() {
            self.search_segment(segment, segment_index, query, &mut collector)?;
        }

        // Read before consuming the collector: `into_sorted` yields only the
        // documents that survived the top-k cut, so counting those would make
        // `total_hits` equal to `limit` for every query that matched more.
        let total_hits = collector.total_hits();

        let scored = collector.into_sorted();
        let hits = scored
            .iter()
            .map(|s| self.to_hit(*s))
            .collect::<Result<Vec<_>>>()?;

        Ok(Results {
            total_hits,
            hits,
            elapsed: started.elapsed(),
        })
    }

    /// Counts matches without ranking them.
    pub fn count(&self, query: &BooleanQuery) -> Result<u64> {
        let mut total = 0u64;

        for segment in self.segments {
            total += self.matching_docs(segment, query)?.len() as u64;
        }

        Ok(total)
    }

    fn to_hit(&self, scored: ScoredDoc) -> Result<Hit> {
        // Map the global id back to a segment by finding the last offset that
        // does not exceed it.
        let segment = self
            .offsets
            .iter()
            .rposition(|offset| *offset <= scored.doc)
            .ok_or_else(|| Error::corrupt(format!("doc {} is in no segment", scored.doc)))?;

        Ok(Hit {
            doc: scored.doc,
            score: scored.score,
            segment,
            local_doc: scored.doc - self.offsets[segment],
        })
    }

    /// Candidate documents in one segment, before scoring.
    fn matching_docs(&self, segment: &Segment, query: &BooleanQuery) -> Result<Vec<DocId>> {
        // `must` clauses are intersected first because they shrink the
        // candidate set fastest; the rarest term is used to seed it.
        let mut candidates: Option<Vec<DocId>> = None;

        for term in &query.must {
            let docs = segment
                .index
                .postings(term.field, &term.term)
                .map(PostingList::doc_ids)
                .unwrap_or_default();

            // A missing required term means the whole segment cannot match.
            if docs.is_empty() {
                return Ok(Vec::new());
            }

            candidates = Some(match candidates {
                None => docs,
                Some(existing) => intersect_sorted(&existing, &docs),
            });

            if candidates.as_ref().is_some_and(Vec::is_empty) {
                return Ok(Vec::new());
            }
        }

        for group in &query.must_any {
            // Union within the group, intersect across groups: at least one
            // term from each group has to match.
            let mut union: Vec<DocId> = Vec::new();
            for term in group {
                if let Some(list) = segment.index.postings(term.field, &term.term) {
                    union = union_sorted(&union, &list.doc_ids());
                }
            }

            if union.is_empty() {
                return Ok(Vec::new());
            }

            candidates = Some(match candidates {
                None => union,
                Some(existing) => intersect_sorted(&existing, &union),
            });

            if candidates.as_ref().is_some_and(Vec::is_empty) {
                return Ok(Vec::new());
            }
        }

        for phrase in &query.phrases {
            let docs = self.phrase_matches(segment, phrase)?;
            if docs.is_empty() {
                return Ok(Vec::new());
            }
            candidates = Some(match candidates {
                None => docs,
                Some(existing) => intersect_sorted(&existing, &docs),
            });
        }

        let mut docs = if let Some(docs) = candidates {
            docs
        } else {
            // No required clauses: the candidate set is the union of the
            // optional ones.
            let mut union: Vec<DocId> = Vec::new();
            for term in &query.should {
                if let Some(list) = segment.index.postings(term.field, &term.term) {
                    union = union_sorted(&union, &list.doc_ids());
                }
            }
            union
        };

        if !query.should.is_empty() && query.must.is_empty() && query.minimum_should_match > 1 {
            docs.retain(|doc| {
                let matched = query
                    .should
                    .iter()
                    .filter(|t| {
                        segment
                            .index
                            .postings(t.field, &t.term)
                            .is_some_and(|l| l.contains(*doc))
                    })
                    .count();
                matched >= query.minimum_should_match
            });
        }

        for term in &query.must_not {
            if let Some(list) = segment.index.postings(term.field, &term.term) {
                docs.retain(|doc| !list.contains(*doc));
            }
        }

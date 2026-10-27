//! Query execution, ranking and snippets.

pub mod collector;
pub mod highlight;
pub mod range;
pub mod searcher;

pub use collector::{CountCollector, ScoredDoc, TopK};
pub use highlight::{HighlightOptions, Span};
pub use range::RangeQuery;
pub use searcher::{BooleanQuery, Hit, PhraseQuery, Results, Searcher, TermQuery};

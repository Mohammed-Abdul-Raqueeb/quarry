//! Query execution, phrase matching and snippets, end to end.

use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::search::highlight::{find_spans, prefix_snippet, snippet, HighlightOptions};
use quarry_core::search::searcher::{has_ordered_run, intersect_sorted, union_sorted};
use quarry_core::{
    Analyzer, BooleanQuery, Document, IndexWriter, PhraseQuery, Searcher, TermQuery, Value,
};
use std::sync::OnceLock;

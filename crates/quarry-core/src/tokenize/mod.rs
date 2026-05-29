//! Turning text into terms.
//!
//! The pipeline is a tokenizer followed by an ordered list of filters, which is
//! the arrangement every mature search engine converges on. The important
//! property is that indexing and querying run the *same* pipeline: if `Running`
//! is indexed as `run`, the query `Running` has to become `run` too, or the
//! document is unfindable. [`Analyzer`] is the single object both sides use so
//! they cannot drift.

pub mod ascii;
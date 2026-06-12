//! Quarry: an embedded full-text search engine.
//!
//! # Why it has no dependencies
//!
//! Every piece a search engine needs — a JSON reader, a stemmer, a varint
//! codec, a CRC, a scoring function — is small and well-specified. Pulling each
//! one in as a crate would mean a dependency tree far larger than the engine,
//! for code that is a few hundred lines apiece. So Quarry builds on `std` and
//! nothing else, and `unsafe` is forbidden workspace-wide.
//!
//! # Shape of the thing
//!
//! ```text
//! Document ──analyze──▶ terms ──▶ InvertedIndex ──flush──▶ Segment (immutable)
//!                                                             │
//!                                       Searcher ◀── merge ◀───┘
//! ```
//!
//! Segments never change once written. New documents go into a new segment and
//! [`index::merge()`] folds them together later, which is what removes the need
//! for locking on the write path.
//!
//! # Example
//!
//! ```
//! use quarry_core::{IndexWriter, Schema, Searcher, BooleanQuery, TermQuery, Analyzer};
//!
//! let schema = Schema::builder()
//!     .text("title")?
//!     .text("body")?
//!     .build()?;
//!
//! let mut writer = IndexWriter::new(schema.clone());
//! writer.add_json_str(r#"{"title":"Rust ownership","body":"Borrowing explained"}"#)?;
//! writer.add_json_str(r#"{"title":"Python typing","body":"Gradual types"}"#)?;
//!
//! let segment = writer.flush().expect("a document was added");
//! let segments = vec![segment];
//!
//! let analyzer = Analyzer::english();
//! let title = schema.field_id("title")?;
//! let term = analyzer.terms("ownership").remove(0);
//!
//! let searcher = Searcher::new(&segments, &schema);
//! let results = searcher.search(&BooleanQuery::new().must(TermQuery::new(title, term)), 10)?;
//!
//! assert_eq!(results.len(), 1);
//! # Ok::<(), quarry_core::Error>(())
//! ```

pub mod dir;
pub mod doc;
pub mod error;
pub mod index;
pub mod json;
pub mod schema;
pub mod score;
pub mod search;
pub mod stats;
pub mod store;
pub mod tokenize;

pub use dir::IndexDir;
pub use doc::{DocId, Document, Value};
pub use error::{Error, Result};
pub use index::{IndexWriter, Segment};
pub use json::Json;
pub use schema::{Field, FieldOptions, FieldType, Schema};
pub use score::{bm25::Bm25, Scorer};
pub use search::{BooleanQuery, Hit, PhraseQuery, Results, Searcher, TermQuery};
pub use stats::IndexStats;
pub use tokenize::{Analyzer, Token};

/// The version of this crate, from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

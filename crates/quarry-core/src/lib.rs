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
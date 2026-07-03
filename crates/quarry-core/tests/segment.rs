//! Segment serialization, corruption detection and merging.
//!
//! A segment is the durability boundary, so the rejection cases matter as much
//! as the round trips. Each corruption below is a failure that actually happens
//! in the field: a process killed mid-write, a damaged byte, a schema changed
//! underneath an existing index.

use quarry_core::index::merge::{merge, merge_with_report, MergePolicy};
use quarry_core::index::segment::{schema_digest, shared_prefix_len, FORMAT_VERSION, MAGIC_END};
use quarry_core::index::Segment;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::{Document, IndexWriter, Value};

//! The on-disk index directory.
//!
//! These touch the filesystem, so each builds its own directory under the
//! system temp dir and removes it afterwards. The names include the process id
//! and a counter so two tests running in parallel cannot collide.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_core::dir::{schema_from_json, schema_to_json, SCHEMA_FILE};
use quarry_core::json::parse;
use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::{Document, IndexDir, IndexWriter, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

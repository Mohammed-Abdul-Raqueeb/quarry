//! `quarry index` — build or extend an index.

use crate::args::Args;
use crate::output::{human_bytes, human_duration, thousands, Style};
use quarry_core::index::WriterConfig;
use quarry_core::json::{self, Json};
use quarry_core::schema::{FieldType, Schema, SchemaBuilder};
use quarry_core::{Error, IndexDir, IndexWriter, Result};
use std::io::Read;
use std::time::Instant;

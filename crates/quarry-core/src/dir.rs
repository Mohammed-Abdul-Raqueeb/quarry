//! An index as a directory on disk.
//!
//! ```text
//! my-index/
//! ├── schema.json       the field definitions, written once
//! ├── 00000001.qseg     a segment
//! └── 00000002.qseg     another
//! ```
//!
//! Segments are numbered, never rewritten, and loaded whole into memory when
//! opened. That last part is the design's main limitation and the reason Quarry
//! is described as *embedded*: an index has to fit in RAM. Memory-mapping the
//! postings instead would lift that, and `docs/architecture.md` explains why it
//! was not done.
//!
//! Writes are atomic by rename. A segment is written to a temporary name and
//! then moved into place, so a process killed mid-write leaves a stray temp file
//! rather than a half-written segment that the reader would have to reject.

use crate::error::{Error, Result};
use crate::index::merge::merge;
use crate::index::Segment;
use crate::json::{self, Json, JsonBuilder};
use crate::schema::{FieldOptions, FieldType, Schema};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

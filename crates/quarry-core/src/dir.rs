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

/// Name of the schema file inside an index directory.
pub const SCHEMA_FILE: &str = "schema.json";
/// Extension given to segment files.
pub const SEGMENT_EXT: &str = "qseg";

/// An index directory.
#[derive(Debug)]
pub struct IndexDir {
    path: PathBuf,
    schema: Schema,
    segments: Vec<Segment>,
}

impl IndexDir {
    /// Creates a new index directory.
    ///
    /// Refuses to proceed if a schema is already there, so an accidental
    /// re-create cannot silently orphan existing segments by replacing the
    /// schema they were written against.
    pub fn create(path: impl AsRef<Path>, schema: &Schema) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;

        let schema_path = path.join(SCHEMA_FILE);
        if schema_path.exists() {
            return Err(Error::schema(format!(
                "{} already holds an index; open it instead",
                path.display()
            )));
        }

        fs::write(&schema_path, schema_to_json(schema).to_string())?;

        Ok(Self {
            path,
            schema: schema.clone(),
            segments: Vec::new(),
        })
    }

    /// Opens an existing index directory, loading every segment.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let schema_path = path.join(SCHEMA_FILE);
        let raw = fs::read_to_string(&schema_path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                Error::schema(format!("{} is not a Quarry index", path.display()))
            } else {
                Error::Io(err)
            }
        })?;

        let schema = schema_from_json(&json::parse(&raw)?)?;

        let mut segments = Vec::new();
        for segment_path in segment_paths(&path)? {
            let bytes = fs::read(&segment_path)?;
            let segment = Segment::read(&bytes)
                .map_err(|err| Error::corrupt(format!("{}: {err}", segment_path.display())))?;
            segment.check_schema(&schema)?;
            segments.push(segment);
        }

        Ok(Self {
            path,
            schema,
            segments,
        })
    }

    /// Opens the index, creating it with `schema` if it is not there yet.
    pub fn open_or_create(path: impl AsRef<Path>, schema: &Schema) -> Result<Self> {
        let path = path.as_ref();
        if path.join(SCHEMA_FILE).exists() {
            Self::open(path)
        } else {
            Self::create(path, schema)
        }
    }

    /// Where the index lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The schema the index was created with.
    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The loaded segments.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Documents across every segment.
    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.segments.iter().map(Segment::doc_count).sum()
    }

    /// Writes a segment into the directory and keeps it loaded.
    pub fn append(&mut self, segment: Segment) -> Result<PathBuf> {
        segment.check_schema(&self.schema)?;

        let next = self.next_segment_number()?;
        let final_path = self.path.join(format!("{next:08}.{SEGMENT_EXT}"));
        let temp_path = self.path.join(format!("{next:08}.{SEGMENT_EXT}.tmp"));

        // Write to a temporary name, then rename: a rename within one
        // directory is atomic, so a reader never sees a partial segment.
        fs::write(&temp_path, segment.write())?;
        fs::rename(&temp_path, &final_path)?;

        self.segments.push(segment);
        Ok(final_path)
    }

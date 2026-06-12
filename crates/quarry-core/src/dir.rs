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

    /// Merges every segment into one, replacing the files on disk.
    ///
    /// The merged segment is written and verified before any input is deleted,
    /// so a failure partway through leaves the original index intact.
    pub fn merge_all(&mut self) -> Result<Option<PathBuf>> {
        if self.segments.len() < 2 {
            return Ok(None);
        }

        let merged = merge(&self.segments, &self.schema)?;
        let existing = segment_paths(&self.path)?;

        let temp_path = self.path.join(format!("merged.{SEGMENT_EXT}.tmp"));
        fs::write(&temp_path, merged.write())?;

        // Read it back before discarding the inputs. A merge bug that produced
        // an unreadable segment would otherwise destroy the whole index.
        let written = fs::read(&temp_path)?;
        Segment::read(&written)?;

        let final_path = self.path.join(format!("{:08}.{SEGMENT_EXT}", 1));

        for path in existing {
            fs::remove_file(path)?;
        }
        fs::rename(&temp_path, &final_path)?;

        self.segments = vec![merged];
        Ok(Some(final_path))
    }

    /// Deletes any `.tmp` files left by an interrupted write.
    pub fn clean_temp_files(&self) -> Result<usize> {
        let mut removed = 0;

        for entry in fs::read_dir(&self.path)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "tmp") {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Total size of the segment files on disk.
    pub fn size_on_disk(&self) -> Result<u64> {
        let mut total = 0;
        for path in segment_paths(&self.path)? {
            total += fs::metadata(path)?.len();
        }
        Ok(total)
    }

    fn next_segment_number(&self) -> Result<u32> {
        let highest = segment_paths(&self.path)?
            .iter()
            .filter_map(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(|stem| stem.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0);

        Ok(highest + 1)
    }
}

/// Segment files in a directory, in numeric order.
///
/// Sorted by name, which is why the numbers are zero-padded: without padding,
/// `10.qseg` would sort before `9.qseg` and merged segments would be stitched
/// together in the wrong document order.
fn segment_paths(path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry_path = entry?.path();
        if entry_path.extension().is_some_and(|ext| ext == SEGMENT_EXT) {
            paths.push(entry_path);
        }
    }

    paths.sort();
    Ok(paths)
}

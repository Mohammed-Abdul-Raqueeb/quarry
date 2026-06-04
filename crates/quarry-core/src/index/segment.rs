//! The on-disk segment format.
//!
//! A segment is one immutable, self-contained index: dictionary, postings,
//! stored fields and statistics in a single file. Immutability is what makes
//! the design simple — nothing is ever updated in place, so there is no
//! locking, no partial-write window, and a reader never sees a half-applied
//! change. New documents go into a new segment, and [`super::merge()`] folds
//! segments together in the background.
//!
//! Layout:
//!
//! ```text
//! magic "QRYS"      4 bytes
//! version           4 bytes
//! body              varint-coded sections:
//!                     schema digest
//!                     doc count
//!                     field statistics
//!                     document field lengths
//!                     term dictionary + postings
//!                     stored fields
//! body checksum     4 bytes  (CRC32 of the body)
//! magic "QRYE"      4 bytes
//! ```
//!
//! The trailing magic is what distinguishes a truncated file from a corrupt
//! one: a write killed partway through has no footer at all, which is a clearer
//! diagnosis than a checksum mismatch.

use super::inverted::{FieldStats, InvertedIndex, TermKey};
use super::posting::PostingList;
use crate::doc::DocId;
use crate::error::{Error, Result};
use crate::schema::{FieldId, Schema};
use crate::store::blob::{BlobReader, BlobWriter};
use crate::store::checksum::{crc32, fnv1a};
use crate::store::docstore::DocStore;
use std::collections::HashMap;

/// Leading magic.
pub const MAGIC_START: &[u8; 4] = b"QRYS";
/// Trailing magic.
pub const MAGIC_END: &[u8; 4] = b"QRYE";
/// Format version this build writes and reads.
pub const FORMAT_VERSION: u32 = 4;
/// Smallest possible well-formed segment: both magics, version and checksum.
pub const MIN_SEGMENT_LEN: usize = 16;

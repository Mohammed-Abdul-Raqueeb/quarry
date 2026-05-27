//! Byte-level reader and writer used by the segment codec.
//!
//! Everything is little-endian and length-prefixed. The pair exists so the
//! segment format is written and read in one place: a mismatch between the two
//! halves is the easiest way to produce a file that decodes into plausible
//! nonsense, and keeping them adjacent makes that mismatch obvious.

use super::checksum::Crc32;
use super::varint;
use crate::error::{Error, Result};

/// Append-only byte buffer with varint and length-prefixed helpers.
#[derive(Debug, Default)]
pub struct BlobWriter {
    bytes: Vec<u8>,
}

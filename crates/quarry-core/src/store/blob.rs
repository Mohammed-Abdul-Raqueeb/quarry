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

impl BlobWriter {
    /// An empty writer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty writer with room for `capacity` bytes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    /// Bytes written so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether nothing has been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The current write offset, for recording in an index.
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Writes one byte.
    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// Writes a little-endian `u32`.
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Writes a little-endian `u64`.
    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Writes a little-endian `f32`.
    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Writes an unsigned varint.
    pub fn varint(&mut self, value: u64) {
        varint::write_u64(&mut self.bytes, value);
    }

    /// Writes a signed varint.
    pub fn svarint(&mut self, value: i64) {
        varint::write_i64(&mut self.bytes, value);
    }

    /// Writes raw bytes with no length prefix.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Writes a varint length followed by the bytes.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.varint(bytes.len() as u64);
        self.bytes.extend_from_slice(bytes);
    }

    /// Writes a varint length followed by the UTF-8 bytes.
    pub fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    /// Writes an ascending `u32` slice as gaps.
    pub fn sorted_u32(&mut self, values: &[u32]) {
        varint::write_sorted_u32(&mut self.bytes, values);
    }

    /// Checksum of everything written so far.
    #[must_use]
    pub fn checksum(&self) -> u32 {
        Crc32::new().update(&self.bytes).finish()
    }

    /// Consumes the writer, returning the buffer.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// The buffer, borrowed.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

/// Cursor over a byte slice, mirroring [`BlobWriter`].
///
/// Every read is bounds-checked and returns [`Error::Corrupt`] rather than
/// panicking: the input is a file that may have been truncated, so a short read
/// is an expected condition, not a bug.
#[derive(Debug, Clone)]
pub struct BlobReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

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

impl<'a> BlobReader<'a> {
    /// A cursor at the start of `bytes`.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Current offset.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Moves to an absolute offset.
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.bytes.len() {
            return Err(Error::corrupt(format!(
                "seek to {pos} past end of {} bytes",
                self.bytes.len()
            )));
        }
        self.pos = pos;
        Ok(())
    }

    /// Bytes left.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// Whether the cursor is at the end.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| Error::corrupt("read length overflows"))?;
        let slice = self.bytes.get(self.pos..end).ok_or_else(|| {
            Error::corrupt(format!("needed {n} bytes, {} left", self.remaining()))
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// Reads one byte.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// Reads a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a little-endian `f32`.
    pub fn f32(&mut self) -> Result<f32> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads an unsigned varint.
    pub fn varint(&mut self) -> Result<u64> {
        let (value, used) = varint::read_u64(&self.bytes[self.pos..])?;
        self.pos += used;
        Ok(value)
    }

    /// Reads an unsigned varint that must fit in a `u32`.
    pub fn varint_u32(&mut self) -> Result<u32> {
        let (value, used) = varint::read_u32(&self.bytes[self.pos..])?;
        self.pos += used;
        Ok(value)
    }

    /// Reads a signed varint.
    pub fn svarint(&mut self) -> Result<i64> {
        let (value, used) = varint::read_i64(&self.bytes[self.pos..])?;
        self.pos += used;
        Ok(value)
    }

    /// Reads `n` raw bytes.
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n)
    }

    /// Reads a length-prefixed byte string.
    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.varint()? as usize;
        self.take(len)
    }

    /// Reads a length-prefixed UTF-8 string.
    pub fn string(&mut self) -> Result<&'a str> {
        let bytes = self.bytes()?;
        std::str::from_utf8(bytes).map_err(|_| Error::corrupt("string is not valid utf-8"))
    }

    /// Reads a gap-encoded ascending `u32` list.
    pub fn sorted_u32(&mut self) -> Result<Vec<u32>> {
        let (values, used) = varint::read_sorted_u32(&self.bytes[self.pos..])?;
        self.pos += used;
        Ok(values)
    }

    /// Verifies a CRC32 over `bytes` against `expected`.
    pub fn verify_checksum(bytes: &[u8], expected: u32) -> Result<()> {
        let actual = Crc32::new().update(bytes).finish();
        if actual == expected {
            Ok(())
        } else {
            Err(Error::corrupt(format!(
                "checksum mismatch: stored {expected:#010x}, computed {actual:#010x}"
            )))
        }
    }
}

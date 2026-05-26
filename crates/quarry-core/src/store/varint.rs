//! LEB128 variable-length integers.
//!
//! Postings are the bulk of an index, and almost every number in one is small:
//! a document gap of 3, a term frequency of 1, a position of 12. A fixed `u32`
//! spends four bytes on each. LEB128 spends one for anything under 128, which
//! is where most of the compression in the segment format comes from.

use crate::error::{Error, Result};

/// Bytes needed to hold the largest `u64` in LEB128.
pub const MAX_VARINT_LEN: usize = 10;

/// Appends `value` to `out` as an unsigned varint.
pub fn write_u64(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            // The high bit means "another byte follows".
            byte |= 0x80;
            out.push(byte);
        } else {
            out.push(byte);
            return;
        }
    }
}

/// Appends `value` as an unsigned varint.
pub fn write_u32(out: &mut Vec<u8>, value: u32) {
    write_u64(out, u64::from(value));
}

/// Reads an unsigned varint, returning the value and how many bytes it used.
pub fn read_u64(input: &[u8]) -> Result<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;

    for (i, &byte) in input.iter().take(MAX_VARINT_LEN).enumerate() {
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            // A value whose last byte is 0x00 but is not the first byte was
            // written with redundant padding, which no honest encoder emits.
            if i > 0 && byte == 0 {
                return Err(Error::corrupt("varint has a redundant trailing zero"));
            }
            return Ok((value, i + 1));
        }
        shift += 7;
    }

    Err(Error::corrupt(
        "varint is unterminated or longer than 10 bytes",
    ))
}

/// Reads an unsigned varint that must fit in a `u32`.
pub fn read_u32(input: &[u8]) -> Result<(u32, usize)> {
    let (value, used) = read_u64(input)?;
    let narrowed =
        u32::try_from(value).map_err(|_| Error::corrupt(format!("{value} does not fit in u32")))?;
    Ok((narrowed, used))
}

/// Maps a signed integer onto an unsigned one so small magnitudes stay small.
///
/// Two's complement makes `-1` into `0xFFFF_FFFF_FFFF_FFFF`, which is the worst
/// case for a varint. Zigzag interleaves the signs instead, so `-1` is 1.
#[must_use]
// The shift-and-xor relies on wrapping: that is what the transformation is.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

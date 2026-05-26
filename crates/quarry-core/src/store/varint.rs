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

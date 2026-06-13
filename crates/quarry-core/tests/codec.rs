//! Varints, checksums and the byte reader/writer pair.
//!
//! These sit under the segment format, so a bug here corrupts an index rather
//! than returning a wrong answer. The decoders are fed deliberately malformed
//! input as well as valid input, because on disk the input is a file that may
//! have been truncated.

use quarry_core::store::blob::{BlobReader, BlobWriter};
use quarry_core::store::checksum::{crc32, fnv1a, Crc32};
use quarry_core::store::varint::{
    encoded_len, read_i64, read_sorted_u32, read_u32, read_u64, write_i64, write_sorted_u32,
    write_u32, write_u64, zigzag_decode, zigzag_encode, MAX_VARINT_LEN,
};

fn round_trip_u64(value: u64) {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, value);
    let (decoded, used) = read_u64(&buffer).expect("should decode");

    assert_eq!(decoded, value, "value did not round trip");
    assert_eq!(used, buffer.len(), "decoder consumed the wrong length");
}

// ---- varint -------------------------------------------------------------

#[test]
fn encodes_zero_in_one_byte() {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, 0);
    assert_eq!(buffer, [0]);
}

#[test]
fn encodes_small_values_in_one_byte() {
    // This is the whole point: a document gap of 3 must not cost four bytes.
    for value in [1u64, 42, 100, 127] {
        let mut buffer = Vec::new();
        write_u64(&mut buffer, value);
        assert_eq!(buffer.len(), 1, "{value} should fit in one byte");
    }
}

#[test]
fn switches_to_two_bytes_at_128() {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, 128);
    assert_eq!(buffer.len(), 2);
}

#[test]
fn round_trips_boundary_values() {
    for value in [
        0,
        1,
        127,
        128,
        255,
        256,
        16_383,
        16_384,
        u64::from(u32::MAX),
        u64::MAX,
    ] {
        round_trip_u64(value);
    }
}

#[test]
fn round_trips_powers_of_two() {
    for shift in 0..64 {
        round_trip_u64(1u64 << shift);
    }
}

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

#[test]
fn round_trips_a_spread_of_values() {
    let mut value = 1u64;
    for _ in 0..40 {
        round_trip_u64(value);
        value = value.wrapping_mul(3).wrapping_add(7);
    }
}

#[test]
fn encodes_the_maximum_u64_in_ten_bytes() {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, u64::MAX);
    assert_eq!(buffer.len(), MAX_VARINT_LEN);
}

#[test]
fn encoded_len_matches_what_was_written() {
    for value in [0u64, 1, 127, 128, 300, 70_000, u64::MAX] {
        let mut buffer = Vec::new();
        write_u64(&mut buffer, value);
        assert_eq!(encoded_len(value), buffer.len(), "length wrong for {value}");
    }
}

#[test]
fn writes_several_values_back_to_back() {
    let mut buffer = Vec::new();
    for value in [1u64, 200, 70_000] {
        write_u64(&mut buffer, value);
    }

    let mut offset = 0;
    for expected in [1u64, 200, 70_000] {
        let (decoded, used) = read_u64(&buffer[offset..]).expect("should decode");
        assert_eq!(decoded, expected);
        offset += used;
    }

    assert_eq!(offset, buffer.len());
}

#[test]
fn rejects_an_empty_buffer() {
    assert!(read_u64(&[]).is_err());
}

#[test]
fn rejects_an_unterminated_varint() {
    // Every byte has its continuation bit set, so the value never ends.
    assert!(read_u64(&[0x80, 0x80, 0x80]).is_err());
}

#[test]
fn rejects_a_varint_longer_than_ten_bytes() {
    assert!(read_u64(&[0x80; 12]).is_err());
}

#[test]
fn rejects_redundant_trailing_zero_padding() {
    // No honest encoder emits this, so accepting it would let two different
    // byte sequences mean the same value.
    assert!(read_u64(&[0x81, 0x00]).is_err());
}

#[test]
fn read_u32_narrows_a_small_value() {
    let mut buffer = Vec::new();
    write_u32(&mut buffer, 70_000);
    assert_eq!(read_u32(&buffer).map(|(v, _)| v).ok(), Some(70_000));
}

#[test]
fn read_u32_rejects_a_value_that_does_not_fit() {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, u64::from(u32::MAX) + 1);
    assert!(read_u32(&buffer).is_err());
}

// ---- zigzag -------------------------------------------------------------

#[test]
fn zigzag_maps_zero_to_zero() {
    assert_eq!(zigzag_encode(0), 0);
}

#[test]
fn zigzag_keeps_small_negatives_small() {
    // Two's complement would make -1 the largest possible u64, which is the
    // worst case for a varint. Zigzag makes it 1.
    assert_eq!(zigzag_encode(-1), 1);
    assert_eq!(zigzag_encode(1), 2);
    assert_eq!(zigzag_encode(-2), 3);
}

#[test]
fn zigzag_round_trips() {
    for value in [0i64, 1, -1, 2, -2, 127, -128, i64::MAX, i64::MIN] {
        assert_eq!(zigzag_decode(zigzag_encode(value)), value, "{value} failed");
    }
}

#[test]
fn signed_varint_round_trips() {
    for value in [0i64, -1, 1, -1000, 1000, i64::MIN, i64::MAX] {
        let mut buffer = Vec::new();
        write_i64(&mut buffer, value);
        assert_eq!(read_i64(&buffer).map(|(v, _)| v).ok(), Some(value));
    }
}

#[test]
fn signed_varint_keeps_minus_one_to_one_byte() {
    let mut buffer = Vec::new();
    write_i64(&mut buffer, -1);
    assert_eq!(buffer.len(), 1);
}

// ---- sorted lists -------------------------------------------------------

#[test]
fn round_trips_an_empty_sorted_list() {
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &[]);
    assert_eq!(read_sorted_u32(&buffer).map(|(v, _)| v).ok(), Some(vec![]));
}

#[test]
fn round_trips_a_sorted_list() {
    let values = vec![1u32, 5, 100, 1000];
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &values);

    assert_eq!(read_sorted_u32(&buffer).map(|(v, _)| v).ok(), Some(values));
}

#[test]
fn gap_encoding_shrinks_a_dense_list() {
    // The property the encoding exists for: adjacent ids cost about a byte
    // each, not four.
    let dense: Vec<u32> = (1_000_000..1_000_200).collect();
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &dense);

    assert!(
        buffer.len() < dense.len() * 2,
        "200 adjacent ids took {} bytes",
        buffer.len()
    );
}

#[test]
fn round_trips_a_list_starting_at_zero() {
    let values = vec![0u32, 1, 2];
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &values);
    assert_eq!(read_sorted_u32(&buffer).map(|(v, _)| v).ok(), Some(values));
}

#[test]
fn round_trips_a_list_with_large_gaps() {
    let values = vec![0u32, 1_000_000, 2_000_000];
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &values);
    assert_eq!(read_sorted_u32(&buffer).map(|(v, _)| v).ok(), Some(values));
}

#[test]
fn round_trips_a_single_element_list() {
    let mut buffer = Vec::new();
    write_sorted_u32(&mut buffer, &[7]);
    assert_eq!(read_sorted_u32(&buffer).map(|(v, _)| v).ok(), Some(vec![7]));
}

#[test]
fn rejects_a_sorted_list_that_ends_early() {
    // A length claiming more entries than the buffer holds.
    let mut buffer = Vec::new();
    write_u32(&mut buffer, 5);
    write_u32(&mut buffer, 1);
    assert!(read_sorted_u32(&buffer).is_err());
}

#[test]
fn rejects_a_sorted_list_whose_gaps_overflow() {
    let mut buffer = Vec::new();
    write_u32(&mut buffer, 2);
    write_u32(&mut buffer, u32::MAX);
    write_u32(&mut buffer, u32::MAX);
    assert!(read_sorted_u32(&buffer).is_err());
}

// ---- checksums ----------------------------------------------------------

#[test]
fn crc32_of_the_standard_check_string() {
    // The published check value for CRC-32/ISO-HDLC.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn crc32_of_empty_input_is_zero() {
    assert_eq!(crc32(b""), 0);
}

#[test]
fn crc32_differs_for_different_input() {
    assert_ne!(crc32(b"hello"), crc32(b"world"));
}

#[test]
fn crc32_detects_a_single_flipped_bit() {
    // The failure the checksum exists to catch.
    assert_ne!(crc32(b"segment"), crc32(b"segmenu"));
}

#[test]
fn crc32_detects_transposed_bytes() {
    assert_ne!(crc32(b"ab"), crc32(b"ba"));
}

#[test]
fn incremental_crc32_matches_the_one_shot() {
    let whole = crc32(b"the quick brown fox");

    let mut incremental = Crc32::new();
    incremental.write(b"the quick ");
    incremental.write(b"brown fox");

    assert_eq!(incremental.finish(), whole);
}

#[test]
fn incremental_crc32_handles_many_small_writes() {
    let data = b"abcdefghijklmnopqrstuvwxyz";
    let mut incremental = Crc32::new();
    for byte in data {
        incremental.write(&[*byte]);
    }
    assert_eq!(incremental.finish(), crc32(data));
}

#[test]
fn crc32_default_matches_new() {
    assert_eq!(Crc32::default().finish(), Crc32::new().finish());
}

#[test]
fn fnv1a_is_deterministic() {
    assert_eq!(fnv1a(b"quarry"), fnv1a(b"quarry"));
}

#[test]
fn fnv1a_differs_for_different_input() {
    assert_ne!(fnv1a(b"title"), fnv1a(b"body"));
}

#[test]
fn fnv1a_of_empty_input_is_the_offset_basis() {
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
}

// ---- BlobWriter / BlobReader --------------------------------------------

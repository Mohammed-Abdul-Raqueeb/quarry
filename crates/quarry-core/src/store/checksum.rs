//! CRC32 (IEEE 802.3), used to detect a truncated or damaged segment.
//!
//! Every segment carries a checksum in its footer. The failure this guards
//! against is mundane and common: a process killed mid-write leaves a file that
//! parses as far as it goes and then returns nonsense. A checksum turns that
//! into one clear error instead of a confusing query result.

/// The reversed IEEE polynomial, as used by zlib, PNG and gzip.
const POLYNOMIAL: u32 = 0xEDB8_8320;

/// Precomputed byte-at-a-time table.
///
/// Built in a `const` block so it lands in the binary rather than being
/// computed on first use, which also keeps the function free of a `OnceLock`.
const TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;

    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            // Shift out the low bit; if it was set, fold in the polynomial.
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ POLYNOMIAL
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }

    table
}

/// CRC32 of a byte slice.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    Crc32::new().update(bytes).finish()
}

/// Incremental CRC32, for checksumming a stream without buffering it.
#[derive(Debug, Clone, Copy)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

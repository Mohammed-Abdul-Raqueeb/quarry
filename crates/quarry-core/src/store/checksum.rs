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

//! The one error type the engine returns.

use std::fmt;

/// Everything that can go wrong inside Quarry.
///
/// Deliberately a single flat enum rather than one error type per module. The
/// call graph is shallow enough that nested errors would mean writing `From`
/// impls all the way up without telling a caller anything they could act on.
#[derive(Debug)]
pub enum Error {
    /// An underlying filesystem or stream failure.
    Io(std::io::Error),
    /// On-disk data did not match the format Quarry expects.
    ///
    /// This is always either a truncated write or a bug in the codec; it is
    /// never something a caller can retry.
    Corrupt(String),
    /// A document did not match the schema it was indexed against.
    Schema(String),
    /// Malformed JSON on input.
    Json(String),
    /// A field was referenced that the schema does not define.
    UnknownField(String),
    /// The segment was written by an incompatible version of the format.
    Version {
        /// The version found on disk.
        found: u32,
        /// The version this build can read.
        expected: u32,
    },
    /// A limit guarding against hostile or runaway input was exceeded.
    Limit(String),
}

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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Corrupt(what) => write!(f, "corrupt index: {what}"),
            Self::Schema(what) => write!(f, "schema error: {what}"),
            Self::Json(what) => write!(f, "invalid json: {what}"),
            Self::UnknownField(name) => write!(f, "unknown field: {name}"),
            Self::Version { found, expected } => {
                write!(f, "segment format v{found} cannot be read by v{expected}")
            }
            Self::Limit(what) => write!(f, "limit exceeded: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

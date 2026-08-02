//! The Quarry command line interface, as a library.
//!
//! The binary is a thin wrapper over [`run`]. Splitting them is what makes the
//! argument parser and the output helpers testable from an integration test
//! rather than only through a subprocess.

pub mod args;
pub mod commands;
pub mod output;

pub use args::{ArgError, Args};
pub use output::Style;

/// Options that consume the next argument.
///
/// The parser needs this list because `--limit 5` is otherwise ambiguous: `5`
/// could be the value or a positional argument.
pub const VALUE_FLAGS: &[&str] = &[
    "index", "i", "schema", "s", "input", "f", "limit", "n", "terms", "t", "batch", "color",
];

/// What the process should exit with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The command succeeded.
    Ok,
    /// The command failed and a message was already printed.
    Failed,
}

impl Outcome {
    /// Whether the command succeeded.
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

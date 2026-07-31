//! Terminal output.
//!
//! Colour goes through one place so it can be switched off in one place.
//! Honouring `NO_COLOR` matters because this output gets piped into files and
//! grepped, where escape sequences are noise.

use std::io::IsTerminal;

/// Whether to emit ANSI escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    enabled: bool,
}

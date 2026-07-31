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

impl Style {
    /// Decides from the environment and whether stdout is a terminal.
    ///
    /// Checks `NO_COLOR` first, per the informal standard: its presence at any
    /// value disables colour.
    #[must_use]
    pub fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Self { enabled: false };
        }
        if std::env::var("TERM").is_ok_and(|t| t == "dumb") {
            return Self { enabled: false };
        }
        Self {
            enabled: std::io::stdout().is_terminal(),
        }
    }

    /// Colour forced on.
    #[must_use]
    pub fn always() -> Self {
        Self { enabled: true }
    }

    /// Colour forced off.
    #[must_use]
    pub fn never() -> Self {
        Self { enabled: false }
    }

    /// Whether colour is on.
    #[must_use]
    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    /// Bold.
    #[must_use]
    pub fn bold(self, text: &str) -> String {
        self.wrap("1", text)
    }

    /// Dimmed, for secondary detail.
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.wrap("2", text)
    }

    /// Green, for success.
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// Yellow, for a warning or a highlighted match.
    #[must_use]
    pub fn yellow(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// Red, for an error.
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.wrap("31", text)
    }

    /// Cyan, for a field name or an identifier.
    #[must_use]
    pub fn cyan(self, text: &str) -> String {
        self.wrap("36", text)
    }
}

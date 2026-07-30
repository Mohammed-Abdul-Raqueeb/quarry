//! Argument parsing.
//!
//! `clap` is the obvious dependency and Quarry has none, so this is hand-rolled.
//! It handles what the CLI actually uses: subcommands, `--flag`, `--key value`,
//! `--key=value`, `-k`, clustered short flags, and `--` to stop parsing. It
//! deliberately does not handle anything else — a bespoke parser that tries to
//! cover every GNU convention is how this kind of module becomes a liability.

use std::collections::HashMap;
use std::fmt;

/// Why the arguments could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// A flag that no command defines.
    Unknown(String),
    /// An option that takes a value was given none.
    MissingValue(String),
    /// A required positional argument was absent.
    MissingArgument(String),
    /// A value that should have been a number was not.
    NotANumber {
        /// The option's name.
        flag: String,
        /// What was given.
        value: String,
    },
    /// No subcommand was given.
    NoCommand,
    /// A subcommand that does not exist.
    UnknownCommand(String),
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(flag) => write!(f, "unknown option: {flag}"),
            Self::MissingValue(flag) => write!(f, "{flag} needs a value"),
            Self::MissingArgument(name) => write!(f, "missing required argument: {name}"),
            Self::NotANumber { flag, value } => {
                write!(f, "{flag} expects a number, got {value:?}")
            }
            Self::NoCommand => f.write_str("no command given"),
            Self::UnknownCommand(name) => write!(f, "unknown command: {name}"),
        }
    }
}

impl std::error::Error for ArgError {}

/// Parsed command line.
#[derive(Debug, Clone, Default)]
pub struct Args {
    /// The subcommand, if one was given.
    pub command: Option<String>,
    /// Positional arguments after the subcommand.
    pub positional: Vec<String>,
    /// Options with values.
    flags: HashMap<String, String>,
    /// Flags given without a value.
    switches: Vec<String>,
}

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

impl Args {
    /// Parses an argument list, excluding the program name.
    ///
    /// `known_value_flags` tells the parser which options consume the next
    /// argument. Without it, `--limit 5` is ambiguous: `5` could be the value
    /// or a positional argument, and there is no way to tell from syntax alone.
    pub fn parse<I, S>(args: I, known_value_flags: &[&str]) -> Result<Self, ArgError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let raw: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut parsed = Self::default();
        let mut i = 0;
        let mut only_positional = false;

        while i < raw.len() {
            let arg = &raw[i];

            if only_positional {
                parsed.push_positional(arg.clone());
                i += 1;
                continue;
            }

            if arg == "--" {
                only_positional = true;
                i += 1;
                continue;
            }

            if let Some(body) = arg.strip_prefix("--") {
                // `--key=value` carries its own value.
                if let Some((key, value)) = body.split_once('=') {
                    parsed.flags.insert(key.to_string(), value.to_string());
                    i += 1;
                    continue;
                }

                if known_value_flags.contains(&body) {
                    let value = raw
                        .get(i + 1)
                        .ok_or_else(|| ArgError::MissingValue(format!("--{body}")))?;
                    parsed.flags.insert(body.to_string(), value.clone());
                    i += 2;
                } else {
                    parsed.switches.push(body.to_string());
                    i += 1;
                }
                continue;
            }

            if let Some(body) = arg.strip_prefix('-') {
                if body.is_empty() {
                    // A bare `-` conventionally means stdin, so it is a value.
                    parsed.push_positional(arg.clone());
                    i += 1;
                    continue;
                }

                // A single short flag may take a value; clustered ones cannot,
                // since there would be no way to say which consumed it.
                if body.len() == 1 && known_value_flags.contains(&body) {
                    let value = raw
                        .get(i + 1)
                        .ok_or_else(|| ArgError::MissingValue(format!("-{body}")))?;
                    parsed.flags.insert(body.to_string(), value.clone());
                    i += 2;
                } else {
                    for ch in body.chars() {
                        parsed.switches.push(ch.to_string());
                    }
                    i += 1;
                }
                continue;
            }

            parsed.push_positional(arg.clone());
            i += 1;
        }

        Ok(parsed)
    }

    fn push_positional(&mut self, value: String) {
        // The first bare word is the subcommand; the rest are its arguments.
        if self.command.is_none() {
            self.command = Some(value);
        } else {
            self.positional.push(value);
        }
    }

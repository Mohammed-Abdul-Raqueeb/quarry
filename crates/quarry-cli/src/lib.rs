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

/// Runs the CLI over an argument list, excluding the program name.
#[must_use]
pub fn run(argv: Vec<String>) -> Outcome {
    if argv.is_empty() || argv.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", help_text());
        return Outcome::Ok;
    }

    if argv.iter().any(|a| a == "--version" || a == "-V") {
        println!("quarry {}", quarry_core::VERSION);
        return Outcome::Ok;
    }

    let parsed = match Args::parse(argv, VALUE_FLAGS) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("error: {err}");
            return Outcome::Failed;
        }
    };

    let style = match parsed.value("color", None) {
        Some("always") => Style::always(),
        Some("never") => Style::never(),
        // `auto`, anything else, or absent: decide from the environment.
        _ => Style::detect(),
    };

    let Some(command) = parsed.command.as_deref() else {
        eprintln!("error: no command given");
        return Outcome::Failed;
    };

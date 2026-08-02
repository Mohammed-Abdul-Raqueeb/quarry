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

    let result = match command {
        "index" => commands::index::run(&parsed, style),
        "search" => commands::search::run(&parsed, style),
        "stats" => commands::stats::run(&parsed, style),
        "merge" => commands::merge::run(&parsed, style),
        "help" => {
            print!("{}", help_text());
            return Outcome::Ok;
        }
        other => {
            eprintln!("error: unknown command {other:?}");
            eprintln!();
            print!("{}", help_text());
            return Outcome::Failed;
        }
    };

    match result {
        Ok(()) => Outcome::Ok,
        Err(err) => {
            eprintln!("{} {err}", style.red("error:"));

            // An I/O failure may be worth retrying; a corrupt index never is,
            // so the distinction is surfaced rather than left to guesswork.
            if err.is_transient() {
                eprintln!(
                    "{} this looks transient; retrying may work",
                    style.dim("note:")
                );
            }

            Outcome::Failed
        }
    }
}

/// The help text, as a string so a test can assert every command appears in it.
#[must_use]
pub fn help_text() -> String {
    format!(
        "\
quarry {version} — an embedded full-text search engine

USAGE
    quarry <command> [options]

COMMANDS
    index     Build or extend an index from newline-delimited JSON
    search    Run a query against an index
    stats     Report what an index contains
    merge     Fold segments together to speed up queries

INDEX
    quarry index --index DIR --schema SPEC [--input FILE]

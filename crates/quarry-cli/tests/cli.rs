//! Argument parsing and output formatting.

use quarry_cli::args::{ArgError, Args};
use quarry_cli::output::{fit, human_bytes, human_duration, thousands, Style};
use quarry_cli::{help_text, VALUE_FLAGS};
use std::time::Duration;

fn parse(args: &[&str]) -> Args {
    Args::parse(args.to_vec(), VALUE_FLAGS).expect("should parse")
}

// ---- subcommands and positionals ----------------------------------------

#[test]
fn the_first_bare_word_is_the_subcommand() {
    assert_eq!(parse(&["search"]).command.as_deref(), Some("search"));
}

#[test]
fn later_bare_words_are_positional() {
    let args = parse(&["search", "rust", "ownership"]);
    assert_eq!(
        args.positional,
        vec!["rust".to_string(), "ownership".to_string()]
    );
}

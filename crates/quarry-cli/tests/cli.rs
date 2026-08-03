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

#[test]
fn an_empty_argument_list_has_no_command() {
    assert!(parse(&[]).command.is_none());
}

#[test]
fn positional_arguments_are_joined_for_an_unquoted_query() {
    // So `quarry search my index .` works without the user remembering quotes.
    let args = parse(&["search", "title:rust", "AND", "ownership"]);
    assert_eq!(args.joined_positional(0), "title:rust AND ownership");
}

#[test]
fn joined_positional_from_a_later_index_skips_the_earlier_ones() {
    let args = parse(&["search", "a", "b", "c"]);
    assert_eq!(args.joined_positional(1), "b c");
}

#[test]
fn joined_positional_past_the_end_is_empty() {
    assert_eq!(parse(&["search"]).joined_positional(5), "");
}

#[test]
fn positional_returns_one_by_index() {
    let args = parse(&["search", "first", "second"]);
    assert_eq!(args.positional(1), Some("second"));
    assert_eq!(args.positional(9), None);
}

#[test]
fn require_positional_errors_when_absent() {
    assert!(matches!(
        parse(&["search"]).require_positional(0, "query"),
        Err(ArgError::MissingArgument(_))
    ));
}

// ---- long options -------------------------------------------------------

#[test]
fn reads_a_long_option_with_a_separate_value() {
    assert_eq!(
        parse(&["search", "--index", "./idx"]).value("index", None),
        Some("./idx")
    );
}

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

#[test]
fn reads_a_long_option_with_an_equals_sign() {
    assert_eq!(
        parse(&["search", "--index=./idx"]).value("index", None),
        Some("./idx")
    );
}

#[test]
fn an_equals_value_may_contain_more_equals_signs() {
    let args = parse(&["search", "--schema=a=b"]);
    assert_eq!(args.value("schema", None), Some("a=b"));
}

#[test]
fn an_equals_value_may_be_empty() {
    assert_eq!(
        parse(&["search", "--index="]).value("index", None),
        Some("")
    );
}

#[test]
fn a_long_option_without_its_value_is_an_error() {
    assert!(matches!(
        Args::parse(vec!["search", "--index"], VALUE_FLAGS),
        Err(ArgError::MissingValue(_))
    ));
}

#[test]
fn an_unknown_long_flag_becomes_a_switch() {
    // It is not consumed as a value flag, so it lands in the switch list where
    // `unknown_switches` can report it.
    assert!(parse(&["search", "--verbose"]).flag("verbose", None));
}

// ---- short options ------------------------------------------------------

#[test]
fn reads_a_short_option_with_a_value() {
    assert_eq!(
        parse(&["search", "-i", "./idx"]).value("index", Some("i")),
        Some("./idx")
    );
}

#[test]
fn a_short_value_option_without_its_value_is_an_error() {
    assert!(Args::parse(vec!["search", "-n"], VALUE_FLAGS).is_err());
}

#[test]
fn a_short_switch_is_recorded() {
    assert!(parse(&["search", "-q"]).flag("quiet", Some("q")));
}

#[test]
fn clustered_short_switches_are_split() {
    let args = parse(&["search", "-abc"]);
    assert!(args.flag("a", Some("a")));
    assert!(args.flag("b", Some("b")));
    assert!(args.flag("c", Some("c")));
}

#[test]
fn a_bare_dash_is_positional() {
    // Conventionally stdin, so it is a value rather than a flag.
    let args = parse(&["index", "-"]);
    assert_eq!(args.positional(0), Some("-"));
}

// ---- value and flag lookup ----------------------------------------------

#[test]
fn value_prefers_the_long_name() {
    let args = parse(&["search", "--index", "long", "-i", "short"]);
    assert_eq!(args.value("index", Some("i")), Some("long"));
}

#[test]
fn value_falls_back_to_the_short_name() {
    assert_eq!(
        parse(&["search", "-i", "short"]).value("index", Some("i")),
        Some("short")
    );
}

#[test]
fn value_or_returns_the_default_when_absent() {
    assert_eq!(
        parse(&["search"]).value_or("index", None, "./default"),
        "./default"
    );
}

#[test]
fn require_errors_when_the_option_is_absent() {
    assert!(parse(&["search"]).require("index", Some("i")).is_err());
}

#[test]
fn require_names_the_missing_option() {
    let err = parse(&["search"])
        .require("index", None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("--index"), "message was: {err}");
}

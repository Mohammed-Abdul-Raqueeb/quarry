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

#[test]
fn flag_is_false_when_absent() {
    assert!(!parse(&["search"]).flag("json", None));
}

#[test]
fn a_flag_with_a_hyphenated_name_is_recognised() {
    assert!(parse(&["search", "--no-snippets"]).flag("no-snippets", None));
}

// ---- numbers ------------------------------------------------------------

#[test]
fn parses_a_numeric_option() {
    assert_eq!(
        parse(&["search", "--limit", "25"])
            .number("limit", None, 10)
            .ok(),
        Some(25)
    );
}

#[test]
fn a_missing_numeric_option_uses_the_default() {
    assert_eq!(parse(&["search"]).number("limit", None, 10).ok(), Some(10));
}

#[test]
fn a_non_numeric_value_is_an_error() {
    assert!(matches!(
        parse(&["search", "--limit", "lots"]).number("limit", None, 10),
        Err(ArgError::NotANumber { .. })
    ));
}

#[test]
fn a_numeric_error_names_the_option_and_the_value() {
    let err = parse(&["search", "--limit", "lots"])
        .number("limit", None, 10)
        .unwrap_err()
        .to_string();

    assert!(err.contains("--limit"), "message was: {err}");
    assert!(err.contains("lots"), "message was: {err}");
}

#[test]
fn a_negative_number_is_rejected_for_a_count() {
    assert!(parse(&["search", "--limit", "-5"])
        .number("limit", None, 10)
        .is_err());
}

#[test]
fn zero_is_an_acceptable_number() {
    assert_eq!(
        parse(&["stats", "--terms", "0"])
            .number("terms", None, 5)
            .ok(),
        Some(0)
    );
}

// ---- the double dash ----------------------------------------------------

#[test]
fn a_double_dash_stops_option_parsing() {
    let args = parse(&["search", "--", "--not-a-flag"]);
    assert_eq!(args.positional(0), Some("--not-a-flag"));
}

#[test]
fn a_double_dash_lets_a_query_start_with_a_hyphen() {
    let args = parse(&["search", "--", "-rust"]);
    assert_eq!(args.joined_positional(0), "-rust");
}

// ---- unknown option reporting -------------------------------------------

#[test]
fn unknown_switches_are_reported() {
    // Silently ignoring `--limit10` means ten thousand results and no clue why.
    let args = parse(&["search", "--json", "--bogus"]);
    assert_eq!(args.unknown_switches(&["json"]), vec!["bogus".to_string()]);
}

#[test]
fn known_switches_are_not_reported() {
    assert!(parse(&["search", "--json"])
        .unknown_switches(&["json"])
        .is_empty());
}

#[test]
fn unknown_value_flags_are_reported() {
    // `batch` takes a value, so it lands in the value map; a command that does
    // not accept it can find it here.
    let args = parse(&["search", "--index", "x", "--batch", "100"]);
    let unknown = args.unknown_flags(&["index"]);

    assert!(unknown.contains(&"batch".to_string()));
}

#[test]
fn a_misspelled_option_that_takes_no_value_shows_up_as_a_switch() {
    // `--colour` is not in VALUE_FLAGS, so it is not consumed as a value
    // option and its argument stays positional. The switch list is where a
    // command finds it to complain about.
    let args = parse(&["search", "--colour", "always"]);

    assert!(args
        .unknown_switches(&["color"])
        .contains(&"colour".to_string()));
    assert_eq!(args.positional(0), Some("always"));
}

// ---- realistic command lines --------------------------------------------

#[test]
fn parses_a_realistic_index_command() {
    let args = parse(&[
        "index",
        "--index",
        "./idx",
        "--schema",
        "title:text,body:text",
        "--input",
        "docs.ndjson",
    ]);

    assert_eq!(args.command.as_deref(), Some("index"));
    assert_eq!(
        args.value("schema", Some("s")),
        Some("title:text,body:text")
    );
}

#[test]
fn parses_a_realistic_search_command() {
    let args = parse(&["search", "-i", "./idx", "-n", "5", "--json", "title:rust"]);

    assert_eq!(args.value("index", Some("i")), Some("./idx"));
    assert_eq!(args.number("limit", Some("n"), 10).ok(), Some(5));
    assert!(args.flag("json", None));
    assert_eq!(args.joined_positional(0), "title:rust");
}

#[test]
fn options_may_come_after_positionals() {
    let args = parse(&["search", "title:rust", "--limit", "3"]);

    assert_eq!(args.joined_positional(0), "title:rust");
    assert_eq!(args.number("limit", None, 10).ok(), Some(3));
}

// ---- human_bytes --------------------------------------------------------

#[test]
fn renders_small_byte_counts_exactly() {
    assert_eq!(human_bytes(0), "0 B");
    assert_eq!(human_bytes(512), "512 B");
}

#[test]
fn renders_kilobytes_with_one_decimal_when_small() {
    assert_eq!(human_bytes(1536), "1.5 KB");
}

#[test]
fn drops_the_decimal_once_the_number_is_large() {
    assert_eq!(human_bytes(1024 * 50), "50 KB");
}

#[test]
fn renders_megabytes_and_gigabytes() {
    assert_eq!(human_bytes(1024 * 1024 * 3), "3.0 MB");
    assert_eq!(human_bytes(1024 * 1024 * 1024 * 2), "2.0 GB");
}

#[test]
fn switches_units_at_the_boundary() {
    assert_eq!(human_bytes(1023), "1023 B");
    assert_eq!(human_bytes(1024), "1.0 KB");
}

// ---- human_duration -----------------------------------------------------

#[test]
fn renders_microseconds() {
    assert_eq!(human_duration(Duration::from_micros(250)), "250µs");
}

#[test]
fn renders_milliseconds() {
    assert_eq!(human_duration(Duration::from_micros(2_500)), "2.5ms");
}

#[test]
fn renders_seconds() {
    assert_eq!(human_duration(Duration::from_millis(2_500)), "2.50s");
}

#[test]
fn renders_a_zero_duration() {
    assert_eq!(human_duration(Duration::from_micros(0)), "0µs");
}

// ---- thousands ----------------------------------------------------------

#[test]
fn leaves_small_numbers_unseparated() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(999), "999");
}

#[test]
fn separates_thousands() {
    assert_eq!(thousands(1_000), "1,000");
    assert_eq!(thousands(1_483_920), "1,483,920");
}

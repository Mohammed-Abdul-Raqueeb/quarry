//! Argument parsing and output formatting.

use quarry_cli::args::{ArgError, Args};
use quarry_cli::output::{fit, human_bytes, human_duration, thousands, Style};
use quarry_cli::{help_text, VALUE_FLAGS};
use std::time::Duration;

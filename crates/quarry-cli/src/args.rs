//! Argument parsing.
//!
//! `clap` is the obvious dependency and Quarry has none, so this is hand-rolled.
//! It handles what the CLI actually uses: subcommands, `--flag`, `--key value`,
//! `--key=value`, `-k`, clustered short flags, and `--` to stop parsing. It
//! deliberately does not handle anything else — a bespoke parser that tries to
//! cover every GNU convention is how this kind of module becomes a liability.

use std::collections::HashMap;
use std::fmt;

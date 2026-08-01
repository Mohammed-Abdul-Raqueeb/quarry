//! `quarry merge` — fold segments into one.

use crate::args::Args;
use crate::output::{human_bytes, human_duration, thousands, Style};
use quarry_core::index::merge::merge_with_report;
use quarry_core::{Error, IndexDir, Result};
use std::time::Instant;

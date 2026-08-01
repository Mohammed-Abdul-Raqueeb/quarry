//! `quarry stats` — report what an index contains.

use crate::args::Args;
use crate::output::{fit, human_bytes, thousands, Style};
use quarry_core::{Error, IndexDir, IndexStats, Result};

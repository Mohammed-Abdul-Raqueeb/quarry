//! `quarry search` — run a query.

use crate::args::Args;
use crate::output::{fit, human_duration, thousands, Style};
use quarry_core::search::HighlightOptions;
use quarry_core::tokenize::Analyzer;
use quarry_core::{Error, IndexDir, Result, Searcher};
use quarry_query::{parse, plan, PlanContext};

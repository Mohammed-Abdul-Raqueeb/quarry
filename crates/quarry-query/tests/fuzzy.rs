//! Edit distance and the AST helpers.

// Exact float comparison is deliberate here: every value is either a
// constant from the source or a small sum chosen to be exactly
// representable, so a tolerance would hide a real regression.
#![allow(clippy::float_cmp)]

use quarry_query::ast::Query;
use quarry_query::fuzzy::{
    auto_distance, bounded_levenshtein, damerau_levenshtein, find_matches, levenshtein, similarity,
    MAX_DISTANCE,
};

// ---- levenshtein --------------------------------------------------------

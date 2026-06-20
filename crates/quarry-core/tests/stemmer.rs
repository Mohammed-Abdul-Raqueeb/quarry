//! Porter stemmer behaviour.
//!
//! The vocabulary cases come from Porter's own published test set. They are
//! grouped by the rule each one exercises, because when a change breaks one
//! group it is almost always that step's condition that is wrong, and a flat
//! list of two hundred words tells you nothing about which.

use quarry_core::tokenize::stemmer::stem;

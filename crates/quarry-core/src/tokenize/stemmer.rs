//! The Porter stemming algorithm.
//!
//! Stemming is what makes a search for `connect` find `connected`,
//! `connecting` and `connection`. Porter's 1980 algorithm is a sequence of
//! suffix-rewriting rules gated on a measure of syllable count; it is not a
//! dictionary, so it is small, fast and language-specific.
//!
//! It is also deliberately crude. `argument` stems to `argum`, and `universal`
//! and `university` both reach `univers`. Those are not bugs to be patched
//! case-by-case: the algorithm trades precision for recall and tiny size, and
//! patching it would break the property that the same input always produces the
//! same stem on both the index and query side.
//!
//! Reference: Porter, "An algorithm for suffix stripping", Program 14(3), 1980.

use super::{Token, TokenFilter};

/// Applies [`stem`] to every token.
#[derive(Debug, Clone, Copy, Default)]
pub struct PorterStemmer;

//! Tokenizers.

use super::{Token, Tokenizer};

/// Splits on anything that is not a letter or digit.
///
/// A full Unicode segmentation implementation would be a large table and a
/// dependency; `char::is_alphanumeric` already covers scripts that separate
/// words with spaces, which is what Quarry claims to support. Scripts that do
/// not — Chinese, Japanese, Thai — need the n-gram tokenizer instead, and
/// `docs/analysis.md` says so rather than leaving it to be discovered.
#[derive(Debug, Clone, Copy)]
pub struct StandardTokenizer {
    /// Characters kept inside a token instead of splitting it.
    ///
    /// Without these, `wasn't` becomes `wasn` + `t` and `e-mail` becomes two
    /// terms. Apostrophes and hyphens are the two that matter in practice.
    pub keep_internal: bool,
}

impl Default for StandardTokenizer {
    fn default() -> Self {
        Self {
            keep_internal: true,
        }
    }
}

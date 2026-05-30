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

impl StandardTokenizer {
    /// A tokenizer that splits on every non-alphanumeric character.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            keep_internal: false,
        }
    }

    /// Whether `ch` continues the current token.
    fn is_token_char(self, ch: char) -> bool {
        if ch.is_alphanumeric() || ch == '_' {
            return true;
        }
        self.keep_internal && matches!(ch, '\'' | '\u{2019}' | '-' | '.')
    }
}

impl Tokenizer for StandardTokenizer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut position = 0u32;
        let mut start: Option<usize> = None;

        for (offset, ch) in input.char_indices() {
            if self.is_token_char(ch) {
                if start.is_none() {
                    start = Some(offset);
                }
            } else if let Some(begin) = start.take() {
                if let Some(token) = trim_token(input, begin, offset, position) {
                    tokens.push(token);
                    position += 1;
                }
            }
        }

        // A token running to the end of the input has no terminating separator.
        if let Some(begin) = start {
            if let Some(token) = trim_token(input, begin, input.len(), position) {
                tokens.push(token);
            }
        }

        tokens
    }
}

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

/// Trims the internal characters off the edges of a candidate token.
///
/// `keep_internal` lets `'`, `-` and `.` through, which also lets in the
/// trailing period of a sentence and the leading dash of a list item. Those are
/// punctuation, not part of the word, so they are stripped here rather than
/// complicating the scanner.
fn trim_token(input: &str, start: usize, end: usize, position: u32) -> Option<Token> {
    let raw = &input[start..end];
    let trimmed = raw.trim_matches(|c: char| matches!(c, '\'' | '\u{2019}' | '-' | '.'));

    if trimmed.is_empty() {
        return None;
    }

    // Offsets must point into the original text for highlighting to work, so
    // the trim is measured rather than assumed.
    let lead = raw.len() - raw.trim_start_matches(['\'', '\u{2019}', '-', '.']).len();

    Some(Token::new(
        trimmed,
        position,
        start + lead,
        start + lead + trimmed.len(),
    ))
}

/// Emits the entire input as one token.
///
/// For keyword fields, where the value is an identifier and splitting it would
/// make an exact lookup impossible.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeywordTokenizer;

impl Tokenizer for KeywordTokenizer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let start = input.find(trimmed).unwrap_or(0);
        vec![Token::new(trimmed, 0, start, start + trimmed.len())]
    }
}

/// Splits on whitespace only, keeping punctuation attached.
#[derive(Debug, Clone, Copy, Default)]
pub struct WhitespaceTokenizer;

impl Tokenizer for WhitespaceTokenizer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        split_whitespace_indices(input)
            .into_iter()
            .enumerate()
            .map(|(position, (offset, word))| {
                Token::new(word, position as u32, offset, offset + word.len())
            })
            .collect()
    }
}

/// Like `str::split_whitespace`, but yielding byte offsets too.
fn split_whitespace_indices(input: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;

    for (offset, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if let Some(begin) = start.take() {
                out.push((begin, &input[begin..offset]));
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }

    if let Some(begin) = start {
        out.push((begin, &input[begin..]));
    }

    out
}

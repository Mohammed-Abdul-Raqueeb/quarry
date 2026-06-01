//! Character n-grams.
//!
//! Two jobs that the word tokenizer cannot do:
//!
//! - **Languages without word separators.** Chinese and Japanese do not put
//!   spaces between words, so splitting on non-alphanumerics yields one
//!   enormous token. Bigrams are the standard cheap answer.
//! - **Substring and typo matching.** Indexing edge n-grams makes prefix
//!   matching a plain term lookup, which is how an as-you-type box stays fast
//!   without a wildcard scan over the dictionary.
//!
//! The cost is index size: every word becomes several terms, so postings grow
//! roughly with the n-gram range. Use it on one field, not everywhere.

use super::{Token, Tokenizer};

/// Emits every character window between `min` and `max` wide.
#[derive(Debug, Clone, Copy)]
pub struct NgramTokenizer {
    /// Shortest window.
    pub min: usize,
    /// Longest window.
    pub max: usize,
}

impl Default for NgramTokenizer {
    fn default() -> Self {
        // Bigrams and trigrams: enough to be discriminating without exploding
        // the dictionary.
        Self { min: 2, max: 3 }
    }
}

impl NgramTokenizer {
    /// An n-gram tokenizer over a window range.
    ///
    /// `min` is clamped to at least 1, and `max` to at least `min`, so a
    /// reversed or zero range degrades to something sensible rather than
    /// producing no tokens at all.
    #[must_use]
    pub fn new(min: usize, max: usize) -> Self {
        let min = min.max(1);
        Self {
            min,
            max: max.max(min),
        }
    }

    /// Fixed-width windows.
    #[must_use]
    pub fn fixed(n: usize) -> Self {
        Self::new(n, n)
    }
}

impl Tokenizer for NgramTokenizer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        // Collected once, because a window needs random access by character
        // index and `char_indices` only walks forwards.
        let chars: Vec<(usize, char)> = input.char_indices().collect();
        let mut tokens = Vec::new();
        let mut position = 0u32;

        for start in 0..chars.len() {
            for width in self.min..=self.max {
                let end = start + width;
                if end > chars.len() {
                    break;
                }

                let begin_byte = chars[start].0;
                let end_byte = chars[end - 1].0 + chars[end - 1].1.len_utf8();
                let text = &input[begin_byte..end_byte];

                // Whitespace inside a window makes it span a word boundary,
                // where it matches nothing useful.
                if text.chars().any(char::is_whitespace) {
                    continue;
                }

                tokens.push(Token::new(text, position, begin_byte, end_byte));
                position += 1;
            }
        }

        tokens
    }
}

/// Emits prefixes anchored at the start of each word.
///
/// For a prefix search box: indexing `search` as `se`, `sea`, `sear`, `searc`,
/// `search` turns "starts with sear" into an exact term lookup.
#[derive(Debug, Clone, Copy)]
pub struct EdgeNgramTokenizer {
    /// Shortest prefix emitted.
    pub min: usize,
    /// Longest prefix emitted.
    pub max: usize,
}

impl Default for EdgeNgramTokenizer {
    fn default() -> Self {
        Self { min: 2, max: 12 }
    }
}

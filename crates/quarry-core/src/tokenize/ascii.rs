//! Accent folding.

use super::{Token, TokenFilter};

/// Strips diacritics so `café` and `cafe` are the same term.
///
/// Users type accents inconsistently — often because their keyboard makes it
/// awkward — so without this, half of them cannot find the document. A full
/// Unicode NFD decomposition would be a large table; this covers the Latin-1
/// and Latin Extended-A ranges, which is where the characters that actually
/// appear in indexed prose live. Anything outside them passes through unchanged
/// rather than being mangled.
#[derive(Debug, Clone, Copy, Default)]
pub struct AsciiFolding;

impl TokenFilter for AsciiFolding {
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token> {
        tokens
            .into_iter()
            .map(|t| {
                // Skip the allocation for the common all-ASCII case.
                if t.text.is_ascii() {
                    return t;
                }
                let folded = fold(&t.text);
                t.with_text(folded)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "asciifolding"
    }
}

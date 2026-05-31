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

impl TokenFilter for PorterStemmer {
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token> {
        tokens
            .into_iter()
            .map(|t| {
                let stemmed = stem(&t.text);
                t.with_text(stemmed)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "porter"
    }
}

/// Reduces an English word to its Porter stem.
///
/// Input is expected to be lowercase already; a word of two characters or fewer
/// is returned unchanged, since every rule would leave it empty.
#[must_use]
pub fn stem(word: &str) -> String {
    // Non-ASCII never matches a rule and would make the byte indexing below
    // unsound, so it is passed through.
    if word.len() <= 2 || !word.is_ascii() {
        return word.to_string();
    }

    let mut w: Vec<u8> = word.as_bytes().to_vec();

    step1a(&mut w);
    step1b(&mut w);
    step1c(&mut w);
    step2(&mut w);
    step3(&mut w);
    step4(&mut w);
    step5a(&mut w);
    step5b(&mut w);

    String::from_utf8(w).unwrap_or_else(|_| word.to_string())
}

fn is_vowel(w: &[u8], i: usize) -> bool {
    match w[i] {
        b'a' | b'e' | b'i' | b'o' | b'u' => true,
        // A `y` is a vowel only when the letter before it is not, which is what
        // separates `sky` (vowel) from `yes` (consonant).
        b'y' => i > 0 && !is_vowel(w, i - 1),
        _ => false,
    }
}

/// Porter's *m*: the number of vowel-consonant sequences in the stem.
///
/// Rules are gated on this as a stand-in for syllable count — it is what stops
/// `sing` losing its `ing`.
fn measure(w: &[u8]) -> usize {
    let mut m = 0;
    let mut i = 0;

    // Skip a leading consonant run.
    while i < w.len() && !is_vowel(w, i) {
        i += 1;
    }

    while i < w.len() {
        // Vowel run.
        while i < w.len() && is_vowel(w, i) {
            i += 1;
        }
        if i >= w.len() {
            break;
        }
        // Consonant run completes one VC pair.
        while i < w.len() && !is_vowel(w, i) {
            i += 1;
        }
        m += 1;
    }

    m
}

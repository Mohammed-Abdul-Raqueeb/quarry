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

/// Whether the stem contains any vowel.
fn has_vowel(w: &[u8]) -> bool {
    (0..w.len()).any(|i| is_vowel(w, i))
}

/// Whether the word ends in a doubled consonant.
fn ends_double_consonant(w: &[u8]) -> bool {
    w.len() >= 2 && w[w.len() - 1] == w[w.len() - 2] && !is_vowel(w, w.len() - 1)
}

/// Porter's *o*: ends consonant-vowel-consonant, final not w, x or y.
///
/// This is the test that decides whether to restore an `e`, so that `hop`
/// becomes `hope` rather than `hop` after `-ing` is removed.
fn ends_cvc(w: &[u8]) -> bool {
    let n = w.len();
    if n < 3 {
        return false;
    }
    !is_vowel(w, n - 1)
        && is_vowel(w, n - 2)
        && !is_vowel(w, n - 3)
        && !matches!(w[n - 1], b'w' | b'x' | b'y')
}

fn ends_with(w: &[u8], suffix: &str) -> bool {
    w.ends_with(suffix.as_bytes())
}

/// Replaces `suffix` with `replacement` when the stem's measure exceeds `min_m`.
fn replace_if(w: &mut Vec<u8>, suffix: &str, replacement: &str, min_m: usize) -> bool {
    if !ends_with(w, suffix) {
        return false;
    }

    let stem_len = w.len() - suffix.len();
    if measure(&w[..stem_len]) > min_m {
        w.truncate(stem_len);
        w.extend_from_slice(replacement.as_bytes());
    }

    // Returns true because the suffix matched, whether or not it was replaced:
    // Porter's steps stop at the first matching rule either way.
    true
}

/// Plurals: `sses` to `ss`, `ies` to `i`, and a trailing `s` dropped.
fn step1a(w: &mut Vec<u8>) {
    // `sses` and `ies` both lose two characters, which is why they share a branch.
    if ends_with(w, "sses") || ends_with(w, "ies") {
        w.truncate(w.len() - 2);
    } else if ends_with(w, "ss") {
        // Left alone: `ss` is not a plural marker.
    } else if ends_with(w, "s") {
        w.truncate(w.len() - 1);
    }
}

/// Past tense and progressive: `eed`, `ed`, `ing`.
fn step1b(w: &mut Vec<u8>) {
    let mut second_pass = false;

    if ends_with(w, "eed") {
        let stem_len = w.len() - 1;
        if measure(&w[..stem_len - 2]) > 0 {
            w.truncate(stem_len);
        }
    } else if ends_with(w, "ed") {
        let stem_len = w.len() - 2;
        if has_vowel(&w[..stem_len]) {
            w.truncate(stem_len);
            second_pass = true;
        }
    } else if ends_with(w, "ing") {
        let stem_len = w.len() - 3;
        if has_vowel(&w[..stem_len]) {
            w.truncate(stem_len);
            second_pass = true;
        }
    }

    // After removing `-ed` or `-ing` the stem can be left malformed, so it is
    // repaired: `conflat` -> `conflate`, `hopp` -> `hop`, `fil` -> `file`.
    if second_pass {
        if ends_with(w, "at") || ends_with(w, "bl") || ends_with(w, "iz") {
            w.push(b'e');
        } else if ends_double_consonant(w) && !matches!(w[w.len() - 1], b'l' | b's' | b'z') {
            w.truncate(w.len() - 1);
        } else if measure(w) == 1 && ends_cvc(w) {
            w.push(b'e');
        }
    }
}

/// Terminal `y` becomes `i` when the stem has a vowel.
fn step1c(w: &mut [u8]) {
    if ends_with(w, "y") && has_vowel(&w[..w.len() - 1]) {
        let last = w.len() - 1;
        w[last] = b'i';
    }
}

/// Double suffixes: `ational` -> `ate`, `tional` -> `tion`, and so on.
fn step2(w: &mut Vec<u8>) {
    const RULES: &[(&str, &str)] = &[
        ("ational", "ate"),
        ("tional", "tion"),
        ("enci", "ence"),
        ("anci", "ance"),
        ("izer", "ize"),
        ("bli", "ble"),
        ("alli", "al"),
        ("entli", "ent"),
        ("eli", "e"),
        ("ousli", "ous"),
        ("ization", "ize"),
        ("ation", "ate"),
        ("ator", "ate"),
        ("alism", "al"),
        ("iveness", "ive"),
        ("fulness", "ful"),
        ("ousness", "ous"),
        ("aliti", "al"),
        ("iviti", "ive"),
        ("biliti", "ble"),
        ("logi", "log"),
    ];

    for (suffix, replacement) in RULES {
        if replace_if(w, suffix, replacement, 0) {
            return;
        }
    }
}

/// Suffix pairs such as `icate` to `ic`, with `ful` and `ness` removed outright.
fn step3(w: &mut Vec<u8>) {
    const RULES: &[(&str, &str)] = &[
        ("icate", "ic"),
        ("ative", ""),
        ("alize", "al"),
        ("iciti", "ic"),
        ("ical", "ic"),
        ("ful", ""),
        ("ness", ""),
    ];

    for (suffix, replacement) in RULES {
        if replace_if(w, suffix, replacement, 0) {
            return;
        }
    }
}

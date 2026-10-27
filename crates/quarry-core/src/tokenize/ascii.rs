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
    fn apply(&self, tokens: &mut Vec<Token>) {
        for token in tokens.iter_mut() {
            // Skip the allocation for the common all-ASCII case.
            if token.text.is_ascii() {
                continue;
            }
            token.text = fold(&token.text);
        }
    }

    fn name(&self) -> &'static str {
        "asciifolding"
    }
}

/// Folds accented Latin characters to their ASCII base.
#[must_use]
pub fn fold(input: &str) -> String {
    let mut out = String::with_capacity(input.len());

    for ch in input.chars() {
        match fold_char(ch) {
            Some(replacement) => out.push_str(replacement),
            None => out.push(ch),
        }
    }

    out
}

/// The ASCII equivalent of one character, if there is one.
///
/// Returns a `&str` rather than a `char` because some characters fold to more
/// than one: `æ` is `ae`, `ß` is `ss`.
fn fold_char(ch: char) -> Option<&'static str> {
    Some(match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'æ' | 'ǣ' => "ae",
        'Æ' => "AE",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ď' | 'đ' | 'ð' => "d",
        'Ď' | 'Đ' | 'Ð' => "D",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĥ' | 'ħ' => "h",
        'Ĥ' | 'Ħ' => "H",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ĵ' => "j",
        'Ĵ' => "J",
        'ķ' => "k",
        'Ķ' => "K",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' => "n",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'œ' => "oe",
        'Œ' => "OE",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => "S",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' => "t",
        'Ţ' | 'Ť' | 'Ŧ' => "T",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ŵ' => "w",
        'Ŵ' => "W",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'Ý' | 'Ÿ' | 'Ŷ' => "Y",
        'ź' | 'ż' | 'ž' => "z",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'þ' => "th",
        'Þ' => "TH",
        // Typographic punctuation that survives tokenization.
        '\u{2019}' | '\u{2018}' => "'",
        '\u{201C}' | '\u{201D}' => "\"",
        '\u{2013}' | '\u{2014}' => "-",
        '\u{00A0}' => " ",
        _ => return None,
    })
}

/// Whether folding would change `input`.
#[must_use]
pub fn needs_folding(input: &str) -> bool {
    input.chars().any(|c| fold_char(c).is_some())
}

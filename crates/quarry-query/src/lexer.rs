//! Turning query text into tokens.

use crate::error::{QueryError, Result};

/// One lexical unit of a query.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// A bare word.
    Word(String),
    /// A quoted string, already unescaped.
    Quoted(String),
    /// A number, for range bounds and boosts.
    Number(f64),
    /// `AND` or `&&`
    And,
    /// `OR` or `||`
    Or,
    /// `NOT`, `!` or a leading `-`
    Not,
    /// `TO`, inside a range
    To,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `:`
    Colon,
    /// `~`
    Tilde,
    /// `^`
    Caret,
    /// `*`
    Star,
    /// `+`, marking a required clause
    Plus,
}

impl Tok {
    /// Whether this token can begin a clause.
    ///
    /// Used to decide where an implicit `OR` goes between adjacent clauses.
    #[must_use]
    pub fn starts_clause(&self) -> bool {
        matches!(
            self,
            Self::Word(_)
                | Self::Quoted(_)
                | Self::Number(_)
                | Self::Not
                | Self::LParen
                | Self::LBracket
                | Self::LBrace
                | Self::Plus
                | Self::Star
        )
    }
}

/// Longest query accepted, in bytes.
///
/// A query is user input reaching a parser, so it gets a bound. The limit is
/// generous for anything typed and small enough that a pathological generated
/// query cannot make planning expensive.
pub const MAX_QUERY_LEN: usize = 4096;

/// Splits query text into tokens.
///
/// Reserved words are matched case-sensitively: `AND` is an operator but `and`
/// is the English word, which someone will eventually search for.
pub fn tokenize(input: &str) -> Result<Vec<Tok>> {
    if input.len() > MAX_QUERY_LEN {
        return Err(QueryError::TooLong {
            len: input.len(),
            max: MAX_QUERY_LEN,
        });
    }

    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        match ch {
            '(' => {
                tokens.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Tok::RParen);
                i += 1;
            }
            '[' => {
                tokens.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Tok::RBracket);
                i += 1;
            }
            '{' => {
                tokens.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Tok::RBrace);
                i += 1;
            }
            ':' => {
                tokens.push(Tok::Colon);
                i += 1;
            }
            '~' => {
                tokens.push(Tok::Tilde);
                i += 1;
            }
            '^' => {
                tokens.push(Tok::Caret);
                i += 1;
            }
            '*' => {
                tokens.push(Tok::Star);
                i += 1;
            }
            '+' => {
                tokens.push(Tok::Plus);
                i += 1;
            }
            '!' => {
                tokens.push(Tok::Not);
                i += 1;
            }
            '-' => {
                // A `-` is negation only where a clause can start; inside a
                // word it belongs to the word, so `well-known` stays one term.
                if tokens.last().is_none_or(|t| !t.starts_clause()) {
                    tokens.push(Tok::Not);
                    i += 1;
                } else {
                    let (word, next) = read_word(&chars, i);
                    i = next;
                    push_word(&mut tokens, word);
                }
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    tokens.push(Tok::And);
                    i += 2;
                } else {
                    return Err(QueryError::Unexpected {
                        found: "&".to_string(),
                        position: i,
                    });
                }
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    tokens.push(Tok::Or);
                    i += 2;
                } else {
                    return Err(QueryError::Unexpected {
                        found: "|".to_string(),
                        position: i,
                    });
                }
            }
            '"' => {
                let (text, next) = read_quoted(&chars, i)?;
                i = next;
                tokens.push(Tok::Quoted(text));
            }
            _ => {
                let (word, next) = read_word(&chars, i);
                if word.is_empty() {
                    return Err(QueryError::Unexpected {
                        found: ch.to_string(),
                        position: i,
                    });
                }
                i = next;
                push_word(&mut tokens, word);
            }
        }
    }

    Ok(tokens)
}

/// Classifies a word as an operator, a number or a term.
fn push_word(tokens: &mut Vec<Tok>, word: String) {
    match word.as_str() {
        "AND" => tokens.push(Tok::And),
        "OR" => tokens.push(Tok::Or),
        "NOT" => tokens.push(Tok::Not),
        "TO" => tokens.push(Tok::To),
        _ => {
            // Only a fully numeric word is a number; `3d` is a term.
            if let Ok(number) = word.parse::<f64>() {
                tokens.push(Tok::Number(number));
            } else {
                tokens.push(Tok::Word(word));
            }
        }
    }
}

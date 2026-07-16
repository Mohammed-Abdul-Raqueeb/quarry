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
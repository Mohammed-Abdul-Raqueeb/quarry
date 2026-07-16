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

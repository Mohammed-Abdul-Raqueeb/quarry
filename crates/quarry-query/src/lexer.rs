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
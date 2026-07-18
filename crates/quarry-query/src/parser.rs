//! Recursive-descent parser for the query language.
//!
//! Grammar, loosest binding first:
//!
//! ```text
//! query   := or
//! or      := and ( ("OR" | implicit) and )*
//! and     := unary ( "AND" unary )*
//! unary   := ("NOT" | "+")? primary ("^" number)?
//! primary := "(" query ")"
//!          | field ":" value
//!          | value
//! value   := word | word "*" | word "~" number? | quoted | range
//! range   := ("[" | "{") bound "TO" bound ("]" | "}")
//! ```
//!
//! Two juxtaposed clauses with no operator are an implicit `OR`, which is what
//! a user typing `rust ownership` means — they want either word, ranked by how
//! many matched, not a demand for both.

use crate::ast::Query;
use crate::error::{QueryError, Result};
use crate::fuzzy;
use crate::lexer::{tokenize, Tok};

/// Deepest nesting the parser accepts.
pub const MAX_DEPTH: usize = 24;

/// Parses query text into an AST.
///
/// The result is already simplified, so a caller never has to handle the
/// redundant single-child nodes the grammar naturally produces.
pub fn parse(input: &str) -> Result<Query> {
    let tokens = tokenize(input)?;

    if tokens.is_empty() {
        return Err(QueryError::Empty);
    }

    let mut parser = Parser {
        tokens,
        pos: 0,
        depth: 0,
    };

    let query = parser.parse_or()?;

    if parser.pos != parser.tokens.len() {
        // Leftover tokens almost always mean an unbalanced paren.
        return match parser.peek() {
            Some(Tok::RParen) => Err(QueryError::UnbalancedParens),
            Some(tok) => Err(QueryError::Unexpected {
                found: describe(tok),
                position: parser.pos,
            }),
            None => Err(QueryError::UnexpectedEnd {
                expected: "end of query".to_string(),
            }),
        };
    }

    Ok(query.simplify())
}

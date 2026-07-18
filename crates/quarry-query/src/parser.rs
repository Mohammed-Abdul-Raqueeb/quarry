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

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, want: &Tok) -> bool {
        if self.peek() == Some(want) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(QueryError::TooDeep { max: MAX_DEPTH });
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn parse_or(&mut self) -> Result<Query> {
        self.enter()?;
        let mut clauses = vec![self.parse_and()?];

        loop {
            if self.eat(&Tok::Or) {
                clauses.push(self.parse_and()?);
                continue;
            }

            // An implicit OR: the next token starts a clause with no operator
            // between them.
            match self.peek() {
                Some(tok) if tok.starts_clause() => {
                    clauses.push(self.parse_and()?);
                }
                _ => break,
            }
        }

        self.leave();

        Ok(if clauses.len() == 1 {
            clauses.remove(0)
        } else {
            Query::Or(clauses)
        })
    }

    fn parse_and(&mut self) -> Result<Query> {
        self.enter()?;
        let mut clauses = vec![self.parse_unary()?];

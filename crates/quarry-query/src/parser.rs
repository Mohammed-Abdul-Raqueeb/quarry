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

        while self.eat(&Tok::And) {
            clauses.push(self.parse_unary()?);
        }

        self.leave();

        Ok(if clauses.len() == 1 {
            clauses.remove(0)
        } else {
            Query::And(clauses)
        })
    }

    fn parse_unary(&mut self) -> Result<Query> {
        if self.eat(&Tok::Not) {
            let inner = self.parse_unary()?;
            return Ok(inner.negate());
        }

        // `+term` marks a clause required. At this level there is nothing to
        // require it against, so it is accepted and ignored; the planner reads
        // the requirement from the AND/OR structure instead.
        self.eat(&Tok::Plus);

        let mut query = self.parse_primary()?;

        if self.eat(&Tok::Caret) {
            let factor = match self.bump() {
                Some(Tok::Number(n)) => n as f32,
                Some(tok) => {
                    return Err(QueryError::Unexpected {
                        found: describe(&tok),
                        position: self.pos - 1,
                    })
                }
                None => {
                    return Err(QueryError::UnexpectedEnd {
                        expected: "a boost factor".to_string(),
                    })
                }
            };
            query = query.boost(factor);
        }

        Ok(query)
    }

    fn parse_primary(&mut self) -> Result<Query> {
        self.enter()?;
        let result = self.parse_primary_inner();
        self.leave();
        result
    }

    fn parse_primary_inner(&mut self) -> Result<Query> {
        match self.peek().cloned() {
            Some(Tok::LParen) => {
                self.pos += 1;
                let inner = self.parse_or()?;
                if !self.eat(&Tok::RParen) {
                    return Err(QueryError::UnbalancedParens);
                }
                Ok(inner)
            }

            // `*:*` or a bare `*` matches everything.
            Some(Tok::Star) => {
                self.pos += 1;
                if self.eat(&Tok::Colon) {
                    self.eat(&Tok::Star);
                }
                Ok(Query::MatchAll)
            }

            Some(Tok::Word(word)) => {
                self.pos += 1;

                // A colon makes the word a field name rather than a term.
                if self.eat(&Tok::Colon) {
                    return self.parse_field_value(&word);
                }

                self.finish_term(None, word)
            }

            Some(Tok::Number(n)) => {
                self.pos += 1;
                // A number used as a term is searched as written.
                self.finish_term(None, format_number(n))
            }

            Some(Tok::Quoted(text)) => {
                self.pos += 1;
                Ok(self.finish_phrase(None, &text))
            }

            Some(Tok::LBracket | Tok::LBrace) => Err(QueryError::Unexpected {
                found: "a range without a field".to_string(),
                position: self.pos,
            }),

            Some(tok) => Err(QueryError::Unexpected {
                found: describe(&tok),
                position: self.pos,
            }),

            None => Err(QueryError::UnexpectedEnd {
                expected: "a term".to_string(),
            }),
        }
    }

    /// Parses whatever follows `field:`.
    fn parse_field_value(&mut self, field: &str) -> Result<Query> {
        match self.peek().cloned() {
            Some(Tok::Word(word)) => {
                self.pos += 1;
                self.finish_term(Some(field.to_string()), word)
            }
            Some(Tok::Number(n)) => {
                self.pos += 1;
                self.finish_term(Some(field.to_string()), format_number(n))
            }
            Some(Tok::Quoted(text)) => {
                self.pos += 1;
                Ok(self.finish_phrase(Some(field.to_string()), &text))
            }
            Some(Tok::Star) => {
                self.pos += 1;
                // `field:*` means "has any value for this field".
                Ok(Query::Prefix {
                    field: Some(field.to_string()),
                    text: String::new(),
                })
            }
            Some(Tok::LBracket) => {
                self.pos += 1;
                self.parse_range(field, true)
            }
            Some(Tok::LBrace) => {
                self.pos += 1;
                self.parse_range(field, false)
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                // `field:(a OR b)` scopes a whole group to one field.
                let inner = self.parse_or()?;
                if !self.eat(&Tok::RParen) {
                    return Err(QueryError::UnbalancedParens);
                }
                Ok(apply_field(inner, field))
            }
            Some(tok) => Err(QueryError::Unexpected {
                found: describe(&tok),
                position: self.pos,
            }),
            None => Err(QueryError::UnexpectedEnd {
                expected: format!("a value for {field}"),
            }),
        }
    }

    /// Applies a trailing `*` or `~` to a bare term.
    fn finish_term(&mut self, field: Option<String>, text: String) -> Result<Query> {
        if self.eat(&Tok::Star) {
            return Ok(Query::Prefix { field, text });
        }

        if self.eat(&Tok::Tilde) {
            // A bare `~` uses the length-based default rather than demanding a
            // number, since that is what a user means by "about this word".
            let distance = match self.peek() {
                Some(Tok::Number(n)) => {
                    let requested = *n;
                    self.pos += 1;
                    let rounded = requested.round().max(0.0) as u32;
                    if rounded > u32::from(fuzzy::MAX_DISTANCE) {
                        return Err(QueryError::BadFuzzyDistance {
                            found: rounded,
                            max: fuzzy::MAX_DISTANCE,
                        });
                    }
                    rounded as u8
                }
                _ => fuzzy::auto_distance(text.chars().count()),
            };

            return Ok(Query::Fuzzy {
                field,
                text,
                distance,
            });
        }

        Ok(Query::Term { field, text })
    }

    /// Applies a trailing `~slop` to a phrase.
    fn finish_phrase(&mut self, field: Option<String>, text: &str) -> Query {
        let terms: Vec<String> = text.split_whitespace().map(ToString::to_string).collect();

        if terms.is_empty() {
            // An empty phrase cannot match, and saying so beats a confusing
            // "no results" with no explanation.
            return Query::MatchNone;
        }

        let mut slop = 0u32;
        if self.eat(&Tok::Tilde) {
            if let Some(Tok::Number(n)) = self.peek() {
                slop = n.round().max(0.0) as u32;
                self.pos += 1;
            }
        }

        // A one-word phrase is just a term, and planning it as a phrase would
        // demand positions the field may not have.
        if terms.len() == 1 && slop == 0 {
            return Query::Term {
                field,
                text: terms.into_iter().next().unwrap_or_default(),
            };
        }

        Query::Phrase { field, terms, slop }
    }

    /// Parses `lower TO upper` plus the closing bracket.
    fn parse_range(&mut self, field: &str, include_lower: bool) -> Result<Query> {
        let lower = self.parse_bound()?;

        if !self.eat(&Tok::To) {
            return Err(QueryError::UnexpectedEnd {
                expected: "TO inside a range".to_string(),
            });
        }

        let upper = self.parse_bound()?;

        let include_upper = if self.eat(&Tok::RBracket) {
            true
        } else if self.eat(&Tok::RBrace) {
            false
        } else {
            return Err(QueryError::UnexpectedEnd {
                expected: "] or } to close the range".to_string(),
            });
        };

        // Caught here rather than at execution time so the user gets told the
        // range is impossible instead of just getting nothing back.
        if let (Some(low), Some(high)) = (lower, upper) {
            if low > high {
                return Err(QueryError::EmptyRange {
                    field: field.to_string(),
                });
            }
        }

        Ok(Query::Range {
            field: field.to_string(),
            lower,
            upper,
            include_lower,
            include_upper,
        })
    }

    /// One end of a range. `*` means unbounded.
    fn parse_bound(&mut self) -> Result<Option<f64>> {
        match self.bump() {
            Some(Tok::Number(n)) => Ok(Some(n)),
            Some(Tok::Star) => Ok(None),
            Some(Tok::Not) => {
                // A negative bound lexes as Not followed by a number, because
                // `-` is ambiguous until this point.
                match self.bump() {
                    Some(Tok::Number(n)) => Ok(Some(-n)),
                    _ => Err(QueryError::UnexpectedEnd {
                        expected: "a number after '-'".to_string(),
                    }),
                }
            }
            Some(Tok::Word(word)) => {
                word.parse::<f64>()
                    .map(Some)
                    .map_err(|_| QueryError::Unexpected {
                        found: word,
                        position: self.pos - 1,
                    })
            }
            Some(tok) => Err(QueryError::Unexpected {
                found: describe(&tok),
                position: self.pos - 1,
            }),
            None => Err(QueryError::UnexpectedEnd {
                expected: "a range bound".to_string(),
            }),
        }
    }
}

/// Pushes a field name down into every leaf that does not have one.
///
/// This is what makes `title:(rust OR go)` mean `title:rust OR title:go`. A
/// leaf that already names a field keeps it, so `title:(body:x)` is respected
/// rather than silently rewritten.
fn apply_field(query: Query, field: &str) -> Query {
    match query {
        Query::Term { field: None, text } => Query::Term {
            field: Some(field.to_string()),
            text,
        },
        Query::Phrase {
            field: None,
            terms,
            slop,
        } => Query::Phrase {
            field: Some(field.to_string()),
            terms,
            slop,
        },
        Query::Fuzzy {
            field: None,
            text,
            distance,
        } => Query::Fuzzy {
            field: Some(field.to_string()),
            text,
            distance,
        },
        Query::Prefix { field: None, text } => Query::Prefix {
            field: Some(field.to_string()),
            text,
        },
        Query::And(children) => Query::And(
            children
                .into_iter()
                .map(|c| apply_field(c, field))
                .collect(),
        ),
        Query::Or(children) => Query::Or(
            children
                .into_iter()
                .map(|c| apply_field(c, field))
                .collect(),
        ),
        Query::Not(child) => Query::Not(Box::new(apply_field(*child, field))),
        Query::Boost { query, factor } => Query::Boost {
            query: Box::new(apply_field(*query, field)),
            factor,
        },
        other => other,
    }
}

fn describe(tok: &Tok) -> String {
    match tok {
        Tok::Word(w) => w.clone(),
        Tok::Quoted(q) => format!("\"{q}\""),
        Tok::Number(n) => format_number(*n),
        Tok::And => "AND".to_string(),
        Tok::Or => "OR".to_string(),
        Tok::Not => "NOT".to_string(),
        Tok::To => "TO".to_string(),
        Tok::LParen => "(".to_string(),
        Tok::RParen => ")".to_string(),
        Tok::LBracket => "[".to_string(),
        Tok::RBracket => "]".to_string(),
        Tok::LBrace => "{".to_string(),
        Tok::RBrace => "}".to_string(),
        Tok::Colon => ":".to_string(),
        Tok::Tilde => "~".to_string(),
        Tok::Caret => "^".to_string(),
        Tok::Star => "*".to_string(),
        Tok::Plus => "+".to_string(),
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

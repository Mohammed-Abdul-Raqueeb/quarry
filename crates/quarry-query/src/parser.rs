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

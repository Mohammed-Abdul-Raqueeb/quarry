# Query language

## Grammar

Loosest binding first:

```
query   := or
or      := and ( ("OR" | "||" | implicit) and )*
and     := unary ( ("AND" | "&&") unary )*
unary   := ("NOT" | "!" | "-" | "+")? primary ("^" number)?
primary := "(" query ")"
         | field ":" value
         | value
         | "*" [":" "*"]
value   := word
         | word "*"
         | word "~" [number]
         | quoted ["~" number]
         | range
range   := ("[" | "{") bound "TO" bound ("]" | "}")
bound   := number | "-" number | "*"
```

## Syntax

| Form | Meaning |
| --- | --- |
| `rust` | the word, in any default (indexed text) field |
| `rust ownership` | implicit `OR` — either word, ranked by how many matched |
| `rust AND ownership` | both words |
| `rust OR ownership` | either word |
| `rust NOT unsafe` | `rust`, excluding documents with `unsafe` |
| `-unsafe` | same as `NOT unsafe` |
| `+rust` | accepted and ignored; see below |
| `(a OR b) AND c` | grouped |
| `title:rust` | scoped to one field |
| `title:(rust OR go)` | a group scoped to one field |
| `"zero cost"` | the words adjacent and in order |
| `"zero cost"~2` | the phrase, allowing two positions of slack |
| `rust~1` | within one edit |
| `rust~` | within an edit distance chosen from the word's length |
| `own*` | terms starting with `own` |
| `title:*` | documents with any value for `title` |
| `rust^2` | the clause's contribution to the score, doubled |
| `year:[2000 TO 2020]` | inclusive range |
| `year:{2000 TO 2020}` | exclusive range |
| `year:[2000 TO *]` | half-open range |
| `*:*` or `*` | every document |

Operators are **case sensitive**: `AND` is an operator, `and` is a word someone will search for.

## Things worth knowing

### An implicit `OR` is what you want

`rust ownership` means "either word, best matches first" — not a demand for both. That matches what
people expect from a search box, and the ranking does the rest: a document with both words scores
higher than one with either.

A bare disjunction gets `minimum_should_match = 1`, so it cannot degenerate into matching every
document.

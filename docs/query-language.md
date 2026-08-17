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

### A bare term searches several fields at once, disjunctively

With `title` and `body` both indexed as text, `rust` becomes "rust in title **or** rust in body".
Under `AND` that stays a disjunction: `rust AND concurrency` finds a document with `rust` in the
body and `concurrency` in the title.

That is the `must_any` clause in `BooleanQuery`. Getting it wrong — one `must` per field — makes the
query demand the word in *every* field, which returns nothing and looks like an empty index.

### `+` is accepted and ignored

`+rust` parses but does nothing. Requirement comes from the `AND`/`OR` structure, not from a
per-clause marker, so there is nothing for `+` to attach to. It is accepted because people type it
out of habit.

### Query text goes through the same analyzer as documents

`Running` is indexed as `run`, so the query has to become `run` too. The planner runs the query
through the same `Analyzer` the documents used, which is why `Running` and `runs` find the same
documents. Porter is a suffix-stripping algorithm, so irregular forms like `ran` do not join them.

The consequence: a query of nothing but stopwords has no terms left. That is an error —
`the query has no searchable terms` — rather than an empty result, because an empty result with no
explanation is the more confusing outcome.

### A prefix or fuzzy term that matches nothing is not an error

`kuber*` on a corpus with no such terms returns zero results with a note, not an error. The query
was well formed; the corpus just does not contain it. Only a query with *nothing to search for* is
an error.

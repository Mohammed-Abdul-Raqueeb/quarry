# Contributing

## Getting started

```bash
git clone https://github.com/Mohammed-Abdul-Raqueeb/quarry.git
cd quarry
cargo test
```

Nothing to install and nothing to download — there are no dependencies, which is also the first
constraint below.

Before opening a pull request:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

That is exactly what CI runs, so a clean local check means a green run.

## The one rule that matters

**No dependencies.** `[workspace.dependencies]` is empty and CI fails if `Cargo.lock` lists anything
but the four workspace crates.

This is the project's defining constraint, not a preference. If something genuinely cannot be done
without a dependency, open an issue first and make the case — it is a conversation about whether the
feature is worth the constraint, not a review comment.

The same applies to `unsafe`, which is `forbid` workspace-wide. CI also greps for
`allow(unsafe_code)` in case someone tries to override it locally.

## Where code goes

| Crate | Rule |
| --- | --- |
| `quarry-core` | Index, analysis, scoring, search, segment format. Knows about nothing else |
| `quarry-query` | Lexer, parser, planner, fuzzy. Knows about `core` |
| `quarry-cli` | The `quarry` binary. Logic lives in the lib half so it is testable |
| `quarry-server` | The `quarry-server` binary, same split |

The dependency direction is one-way and worth keeping that way: `core` must stay usable as an
embedded library without pulling in a query parser.

## Style

Formatting is `rustfmt`'s job; do not argue with it in review. Beyond that:

**Comments explain why, not what.** If a line needs a comment to say what it does, rename something
instead. The comments worth writing are the ones that stop someone "fixing" a deliberate choice —
why positions are preserved when a stopword is dropped, why BM25's IDF has a `+1` inside the
logarithm, why the segment reader checks the trailing magic before the version.

**Document the limits, not just the behaviour.** The Porter stemmer over-stems `universal` and
`universe` to the same stem. That is in the doc comment and in a test, because someone will
eventually find it and need to know whether it is a bug.

**Prefer a named function to a long closure.** Stack traces get much more useful.

**No `any`-equivalents.** No `unwrap` in library code; return a `Result`. `expect` is fine in tests
and in a binary's `main`, where a panic is the correct response.

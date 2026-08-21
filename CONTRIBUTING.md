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

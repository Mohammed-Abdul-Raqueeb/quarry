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

## Tests

Integration tests in `crates/*/tests/`, which exercise the public API. There are 931 of them and the
suite runs in under a second, so there is no reason not to run it.

One behaviour per test, named as a sentence about the behaviour:

```rust
// good
#[test]
fn a_truncated_segment_is_reported_as_truncated_not_corrupt() { … }

// not good
#[test]
fn test_segment_read() { … }
```

**A bug fix needs a test that fails before the fix.** That is the only way to know it is actually
fixed and stays fixed. Two of the bugs found while building this were silent — a query returning
nothing, and a distance computed too low — and neither would have been noticed without a test that
pinned the correct answer.

**Test the rejections.** For anything that reads external input — the JSON parser, the segment
reader, the HTTP layer, the query lexer — the invalid cases matter as much as the valid ones. The
segment tests flip every byte of a small segment and assert each flip is caught.

**Correcting a test to match the code is only legitimate when the code is right.** If a test fails,
work out which side is wrong before changing either. Several stemmer test expectations in this
codebase were wrong and were corrected only after hand-tracing Porter's published rules; the commit
messages say so.

## Benchmarks

`cargo bench` prints a table. If you change something on a hot path, include before and after
numbers in the pull request.

Be careful with the generator. An earlier version used a fifty-word vocabulary, which made every
posting list enormous and the dictionary tiny, and reported a 0.1% merge saving that was an artefact
of the benchmark rather than a fact about the code. A benchmark that measures the wrong thing is
worse than none.

## Commits

Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `perf:`, `chore:`, `ci:`,
`build:`, `style:`.

One logical change per commit. Say *why* in the body when the what is not obvious from the diff —
particularly for a fix, where the interesting part is how the bug was possible.

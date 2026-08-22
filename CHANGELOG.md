# Changelog

Notable changes to Quarry. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-08-22

### Added

- **HTTP server** (`quarry-server`): `/health`, `/stats`, `/schema`, `GET /search` and
  `POST /search`, on `std::net` with a fixed thread pool. No async runtime, because a request is
  CPU-bound once the index is in memory.
- **Highlighted snippets**, built by re-analyzing the stored original text so the word the user
  typed is the word they see marked.
- **Fuzzy and prefix queries**, expanded against the real dictionary with a cap of 64 terms per
  clause.
- **Index directory** (`IndexDir`): segments numbered on disk, written by atomic rename, with a
  schema file and temp-file cleanup.
- **Segment merging**, with a policy that prefers the smallest candidates and a dry-run mode that
  reports what a merge would save.
- **Benchmarks** (`cargo bench`) over a Zipf-distributed synthetic corpus.

### Changed

- **Crawler of a bug, fixed: a required multi-field term is now a disjunction.** A bare term
  searches every default text field, and those expansions were each becoming a separate `must`
  clause — so `rust AND concurrency` demanded both words in *both* title and body and returned
  nothing. Added a `must_any` clause group: union within a group, intersect across groups.
- **`total` now counts every match, not the number returned.** `Results::total_hits` was being set
  from the collector's output rather than its running count, so `limit=1` always reported one match.
- **Banded edit distance no longer under-reports.** `bounded_levenshtein` left the cell on the
  band's lower edge uninitialised and read a stale value from two rows earlier, so some pairs came
  back closer than they are. Found by cross-checking it against the unbounded implementation.
- Segment format is version 4: front-coded dictionary and a CRC32 footer.
- The stemmer passes non-ASCII input through unchanged rather than attempting ASCII byte rules on it.

## [0.3.0] - 2026-07-18

### Added

- **Query language**: lexer, recursive-descent parser and a planner that resolves fields, analyzes
  query text with the index's own analyzer, and expands against the dictionary.
- Booleans with precedence, implicit `OR`, phrases with slop, field scoping over groups, boosts and
  numeric range parsing.
- Edit distance: Levenshtein, a banded bounded variant, and Damerau for transpositions.
- An unknown field now suggests the closest schema field by edit distance.

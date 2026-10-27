# Changelog

Notable changes to Quarry. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-10-27

### Added

- **Numeric ranges execute.** `year:[2000 TO 2020]`, `{2000 TO 2020}` and half-open forms now
  return results instead of being parsed and dropped. `RangeQuery` carries the bounds and their
  inclusivity, `BooleanQuery::range` adds one to a query, and the planner resolves a parsed range
  into an executable clause.
  - Answered by scanning the field's dictionary, since a numeric field is indexed as the canonical
    decimal form of its value and that dictionary *is* the sorted set of values present. No
    separate numeric structure.
  - Cost is linear in the field's **distinct values**, not in corpus size. Free for a year or a
    rating, a full dictionary pass for a float with a million distinct values.
  - Ranges run last among the required clauses, so a term narrows the candidate set first.
  - An empty range — crossed bounds, or equal bounds with either end excluded — is rejected at plan
    time rather than returning nothing.
  - A **negated** range is still not applied. The executor intersects range results and cannot
    subtract them, so `NOT year:[2000 TO 2020]` is reported as a note and dropped, because dropping
    it silently would turn an exclusion into no filter at all.
- `Analyzer::analyze_into` and `Tokenizer::tokenize_into`, which write into a caller's buffer so
  repeated analysis reuses one allocation. `tokenize_into` has a default implementation, so no
  existing `Tokenizer` has to change.
- A benchmark that tokenizes without filters, giving the other end of the subtraction that shows
  what the filter chain costs.

### Changed

- **Breaking: `TokenFilter::filter` is now `TokenFilter::apply`, and mutates in place.** The old
  signature took a `Vec<Token>` and returned a new one, so a five-filter chain allocated five
  vectors per field value. Implementors must rename the method and mutate `&mut Vec<Token>`.
- `IndexWriter` keeps one token buffer for its lifetime instead of allocating per field value.
- Measured effect of the two changes above, both sides timed in one sitting: analyzing a 6-word
  title 1.8 µs → 1.4 µs, a 500-word body 186 µs → 159 µs, indexing 1,000 documents ~50 ms → ~46 ms.
  `docs/performance.md` says why that is 15–20% and not the several-fold cut the allocation count
  suggests — stemming is half the filter chain's cost on a long body and is untouched by this.
- `docs/performance.md` no longer quotes a rate for the 10,000-document indexing case. Its samples
  spread about threefold and the median disagrees with the linear extrapolation from 1,000
  documents; the range and the reason are reported instead.
- `docs/analysis.md` documents the `TokenFilter` contract an implementor has to honour: preserve
  each token's byte offsets, leave the position gap a dropped token creates, stay deterministic.

### Fixed

- **A length-equality shortcut in the stemmer stopped stemming every `-ly` word.** Rewriting
  `PorterStemmer` to truncate in place made it tempting to skip the substitution when the stem is
  the same length as the word; step 1c turns a terminal `y` into an `i`, so `quickly` → `quickli`
  is exactly that case. Caught by an existing test, and now pinned by its own.

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

### Changed

- Positions are preserved rather than renumbered when a stopword is dropped, so a phrase cannot
  silently match across the gap a removed word leaves.

## [0.2.0] - 2026-06-20

### Added

- **BM25 scoring** with tunable `k1` and `b`, behind a `Scorer` trait, alongside TF-IDF for
  comparison.
- **Bounded top-k collector**: a min-heap of size `k`, so memory is bounded by the limit rather than
  by the result set.
- **Segment format**: an immutable, checksummed file with gap-encoded postings and a stored-field
  section.
- Phrase matching over positions, with slop.
- Accent folding and an n-gram tokenizer.

### Changed

- Terms are keyed by `(field, term)` rather than filtered by field at query time, because a shared
  posting list cannot give a correct per-field document frequency.

## [0.1.0] - 2026-05-24

First working version.

### Added

- Inverted index with term frequencies and positions.
- Analysis pipeline: tokenizer, lowercasing, stopwords and a Porter stemmer, composed into an
  `Analyzer` shared by indexing and querying.
- Schema with text, keyword and numeric field types.
- A hand-written JSON reader and writer, a LEB128 varint codec and a table-driven CRC32 — the
  beginnings of the no-dependency constraint.
- `quarry index` and `quarry search`.

[0.5.0]: https://github.com/Mohammed-Abdul-Raqueeb/quarry/releases/tag/v0.5.0
[0.4.0]: https://github.com/Mohammed-Abdul-Raqueeb/quarry/releases/tag/v0.4.0
[0.3.0]: https://github.com/Mohammed-Abdul-Raqueeb/quarry/releases/tag/v0.3.0
[0.2.0]: https://github.com/Mohammed-Abdul-Raqueeb/quarry/releases/tag/v0.2.0
[0.1.0]: https://github.com/Mohammed-Abdul-Raqueeb/quarry/releases/tag/v0.1.0

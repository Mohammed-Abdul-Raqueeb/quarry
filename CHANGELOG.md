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

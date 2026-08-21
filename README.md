# Quarry

A full-text search engine in Rust, with **no dependencies**. Inverted index, BM25 ranking, a query
language, an embeddable library, a CLI and an HTTP server — built on `std` and nothing else.

[![CI](https://github.com/Mohammed-Abdul-Raqueeb/quarry/actions/workflows/ci.yml/badge.svg)](https://github.com/Mohammed-Abdul-Raqueeb/quarry/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.82%2B-orange)
![Dependencies](https://img.shields.io/badge/dependencies-0-brightgreen)
![Unsafe](https://img.shields.io/badge/unsafe-forbidden-blue)
![License](https://img.shields.io/badge/license-MIT-green)

---

## Why zero dependencies

Every piece a search engine needs is small and well specified: a JSON reader, a stemmer, a varint
codec, a CRC, a scoring function. Pulling each one in as a crate would mean a dependency tree
several times larger than the engine itself, for code that is a few hundred lines apiece.

So Quarry writes them. The whole workspace compiles from `std`, `unsafe` is forbidden
workspace-wide, and `cargo build` has nothing to download. The parts that would normally be
dependencies:

| Normally a crate | Here | Why it was worth writing |
| --- | --- | --- |
| `serde_json` | `json.rs` | Only needs a reader and a writer, with a depth cap for hostile input |
| `rust-stemmers` | `tokenize/stemmer.rs` | Porter's algorithm is ~250 lines of suffix rules |
| `integer-encoding` | `store/varint.rs` | LEB128 plus zigzag is under 100 lines |
| `crc32fast` | `store/checksum.rs` | A table-driven CRC32 is one `const fn` |
| `clap` | `cli/args.rs` | The CLI uses six option forms, not sixty |
| `axum` + `tokio` | `server/http.rs` | Requests are CPU-bound once the index is in memory, so threads equal to cores is the right shape |
| `strsim` | `query/fuzzy.rs` | Banded Levenshtein, which a generic crate would not specialise |

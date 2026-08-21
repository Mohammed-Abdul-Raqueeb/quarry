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

## What it does

- **Indexing** — an inverted index with positions, term frequencies and per-field length
  statistics, flushed into immutable segments.
- **Ranking** — BM25 with tunable `k1` and `b`, plus TF-IDF for comparison, behind a `Scorer` trait.
- **Analysis** — a tokenizer, lowercasing, accent folding, stopwords and a Porter stemmer, composed
  into an `Analyzer` that both indexing and querying share so they cannot drift.
- **Query language** — booleans, phrases with slop, field scoping, prefixes, fuzzy terms, boosts and
  ranges, with a lexer, a recursive-descent parser and a planner that resolves against the real
  dictionary.
- **Storage** — a checksummed segment format with a front-coded dictionary and gap-encoded postings,
  plus background merging.
- **Interfaces** — an embeddable library, a CLI, and an HTTP server with a fixed thread pool.

## Try it

```bash
git clone https://github.com/Mohammed-Abdul-Raqueeb/quarry.git
cd quarry
cargo build --release

cat > docs.ndjson <<'EOF'
{"title":"Rust ownership","body":"The borrow checker enforces ownership at compile time","tag":"rust","year":2021}
{"title":"Zero cost abstractions","body":"Iterators compile to the same code you would write by hand","tag":"rust","year":2019}
{"title":"Fearless concurrency","body":"Rust prevents data races by tracking which thread owns what","tag":"rust","year":2020}
{"title":"Python type hints","body":"Gradual typing lets you annotate incrementally","tag":"python","year":2022}
EOF

./target/release/quarry index --index ./idx \
  --schema 'title:text,body:text,tag:keyword,year:numeric' \
  --input docs.ndjson

./target/release/quarry search --index ./idx 'rust AND concurrency'
```

```
1 match in 14µs

  1. 2.3434  #2
     title: Fearless [concurrency]
     body: [Rust] prevents data races by tracking which thread owns what
     tag: rust
     year: 2020
```

# Performance

Numbers from `cargo bench` on the machine they were measured on. Reproduce them before trusting
them — the absolute values are hardware, and only the *ratios* are properties of the code.

```bash
cargo bench
```

## The corpus

The benchmark generates documents from a Zipf-distributed vocabulary: eight very common words, a
fifty-word head, and a synthesised tail of about 20,000 terms. Each document has a six-word title
and a sixty-word body.

The distribution is the point. A fixed small vocabulary makes every posting list enormous and the
dictionary tiny, which hides the single biggest factor in query cost — how selective a term is — and
leaves merging and front-coding with nothing to collapse. An earlier version of this benchmark did
exactly that and reported a 0.1% merge saving, which was an artefact of the generator rather than a
fact about the code.

## Analysis

| Operation | Median |
| --- | --- |
| Analyze a 6-word title | 1.7 µs |
| Analyze a 500-word body | 189 µs |
| Stem one word | 114 ns |

About 380 ns per term through the full English pipeline. That is more than it needs to be: every
filter allocates a fresh `Vec<Token>` and a fresh `String` per token, so a five-filter chain does
five allocations per token.

A single mutable pass would be several times faster. It has not been done because the filter chain's
shape — each filter a small, independently testable `TokenFilter` — is what makes the analyzer easy
to reason about and to extend, and indexing is not the bottleneck for a search workload. It is a
known, deliberate cost rather than an oversight.

## Indexing

| Documents | Median | Rate |
| --- | --- | --- |
| 1,000 | 50.8 ms | ~19,700 docs/s |
| 10,000 | 519 ms | ~19,250 docs/s |

Linear, as it should be: an insert is an analyze plus a `BTreeMap` lookup and a push.

2,000 documents produce 7,543 dictionary entries and 780 bytes per document on disk. The per-document
figure is dominated by positions — a 66-word document stores 66 position entries — which is the price
of phrase matching. Turning positions off for a field roughly halves its postings.

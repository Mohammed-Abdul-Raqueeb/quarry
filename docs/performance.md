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

## Segment codec

| Operation | Median |
| --- | --- |
| Serialize a 5,000-document segment | 21.6 ms |
| Parse a 5,000-document segment | 42.7 ms |

3.86 MB, 13,530 dictionary entries. Parsing costs about twice what writing does, because reading
rebuilds the `BTreeMap` and allocates a `String` per term while writing only walks one.

That matters because parsing happens on `IndexDir::open` — a cold start pays it once for every
segment, which is another reason to keep the segment count low.

## Queries

10,000 documents, one segment:

| Query | Median |
| --- | --- |
| Rare term | 307 ns |
| Common term (matches about half the corpus) | 1.88 ms |
| Two-term `AND`, one rare | 3.9 µs |
| Two-term `OR`, both common | 3.58 ms |
| Two-term phrase | 2.09 ms |
| Count without ranking | 3.6 µs |

Three things worth reading off this table.

**Selectivity is everything.** A rare term is roughly 6,000× faster than a common one. Cost is
proportional to the number of matching documents, because each one has to be scored — not to the
size of the corpus.

**Intersection order works.** The two-term `AND` runs in 3.9 µs, far faster than the common term
alone at 1.88 ms, because the rare term's short posting list becomes the candidate set and the
common term is then only probed for those few documents. A planner that intersected in the other
order would pay the full 1.88 ms.

**Counting is nearly free** — 3.6 µs against 1.88 ms — because it skips scoring entirely. Use
`Searcher::count` when you need a total and not a ranking.

## The bounded collector

| Limit | Median |
| --- | --- |
| 10 | 1.87 ms |
| 100 | 1.92 ms |
| 1,000 | 2.18 ms |

A hundredfold increase in the limit costs 17% more time. That is the point of the min-heap: the work
is proportional to the number of *matches*, not to the requested limit, and memory is bounded by
`k` rather than by the result set.

Sorting every match and truncating would allocate proportional to the matches — on a query matching
a million documents for the top ten, that allocation would be the entire cost of the query.

## Merging

| Operation | Median |
| --- | --- |
| Merge 4 × 2,500-document segments | 65.5 ms |

7.78 MB → 7.62 MB, with 26,187 dictionary entries collapsed.

The byte saving is modest at 2.1% because postings dominate and they do not shrink — the same
postings are rewritten under new ids. The win is the 26,187 duplicate dictionary entries removed,
which is what a query actually walks: four segments means four dictionary lookups per term, one
means one.

So merge for read latency, not for disk space.

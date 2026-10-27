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
| Analyze a 6-word title | 1.4 µs |
| Analyze a 500-word body | 159 µs |
| Tokenize a 500-word body, no filters | 41 µs |
| Stem one word | 117 ns |

About 318 ns per term through the full English pipeline, of which tokenizing is 82 ns and the
five-filter chain is the remaining 236 ns.

### What filtering in place bought

The filters used to take a `Vec<Token>` and return a new one, so a five-filter chain allocated five
vectors per field value and a `String` for every token `Lowercase` touched. They now take
`&mut Vec<Token>`: `Lowercase` rewrites ASCII in place, `LengthFilter` and `StopwordFilter` use
`retain`, `AsciiFolding` skips tokens that are already ASCII, and `PorterStemmer` truncates when the
stem is a prefix of the word.

Both sides measured in one sitting on one machine, from a worktree at the parent commit, because the
absolute numbers here move by more than the effect does between sessions:

| | Before | After |
| --- | --- | --- |
| 6-word title | 1.8 µs | 1.4 µs |
| 500-word body | 186 µs | 159 µs |

So roughly 20% off a short value and 15% off a long one — not the several-fold cut the allocation
count suggests. Two things are worth taking from that.

The short value gains more because the five vector allocations are a fixed cost per field value, and
a title amortises them over six tokens rather than five hundred. Titles and keyword fields are where
this shows up.

The long value gains less because stemming dominates and the change barely touches it: at 117 ns a
word, 500 words is 59 µs, about half of what the whole filter chain costs. `stem` builds a
`Vec<u8>`, rewrites it through eight steps and converts it back to a `String`, and no amount of care
in the chain around it changes that. Making the filters allocation-free was worth doing and was not
where the time was.

Stemming in place against a buffer owned by the filter is the next step and has not been taken:
Porter's steps truncate and re-append, so the rules read clearly against a growable buffer and much
less clearly against a borrowed one. Analysis is now within about 4x of the bare tokenizer, and
indexing is not the bottleneck for a search workload.

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

## Rules of thumb

- Query cost tracks matching documents, not corpus size.
- Put the most selective clause in a conjunction first; the planner does not reorder for you.
- Positions roughly double a field's postings. Turn them off for fields you will never
  phrase-match.
- Keep the segment count low. Each one adds a dictionary lookup per term and a parse on open.
- Use `count` rather than `search` when you only need a total.
- Indexing runs around 19,000 documents per second per core, so a million documents is about a
  minute.

## Honest unknowns

- **No large-corpus numbers.** Everything here is 10,000 documents or fewer. The in-memory design
  means behaviour past a few tens of millions of documents is untested and bounded by RAM.
- **Single-threaded queries.** The server runs requests in parallel, but one query uses one thread.
  Segments are independent and could be searched concurrently; they are not.
- **No profile-guided work.** Nothing here has been optimised against a profiler. The analyzer
  allocation cost above is the one known hot spot, and it is unaddressed on purpose.

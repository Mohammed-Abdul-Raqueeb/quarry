# Architecture

What Quarry does, why it is shaped this way, and what each choice cost. The code says what it does;
this says what the alternatives were.

## The shape of the thing

```
                 ┌──────────────────────────────────────────┐
  Document ──────▶ Analyzer                                 │
  (JSON)         │   tokenize → lowercase → fold → stop →   │
                 │   stem                                   │
                 └───────────────┬──────────────────────────┘
                                 │ terms + positions
                                 ▼
                 ┌──────────────────────────────────────────┐
                 │ InvertedIndex          (in memory)       │
                 │   BTreeMap<(field, term), PostingList>   │
                 └───────────────┬──────────────────────────┘
                                 │ flush
                                 ▼
                 ┌──────────────────────────────────────────┐
                 │ Segment                (immutable file)  │
                 │   front-coded dictionary                 │
                 │   gap-encoded postings                   │
                 │   stored fields                          │
                 │   CRC32 footer                           │
                 └───────────────┬──────────────────────────┘
                                 │
         merge ◀─────────────────┤
                                 ▼
  Query ──lex──▶ Tok ──parse──▶ Query AST ──plan──▶ BooleanQuery
                                                        │
                                                        ▼
                                                   Searcher
                                                   TopK collector
                                                        │
                                                        ▼
                                                      Hits
```

## Segments are immutable

Everything else follows from this. A flush writes a new segment file; nothing is ever updated in
place.

What it buys:

- **No locking on the write path.** A reader holds a segment that cannot change underneath it.
- **No partial-write window.** A segment is written to a temporary name and renamed into place, and
  a rename within one directory is atomic. A process killed mid-write leaves a stray `.tmp` file,
  not a half-valid segment.
- **Cheap concurrency.** Any number of threads can search the same segments with no coordination,
  which is what lets the server use a plain thread pool.

What it costs:

- **No deletes or updates.** Removing a document means reindexing without it. A tombstone bitmap per
  segment is the usual answer and is not implemented.
- **Segment accumulation.** Every flush adds a file, and a query visits each one. Merging trades
  write work for read speed, and `MergePolicy` decides when.

## The index is held in memory

`IndexDir::open` reads every segment into RAM and keeps it there. This is the biggest limitation in
the project and the one most worth understanding before adopting it.

The alternative is memory-mapping the postings and letting the OS page them, which is what Lucene
and Tantivy do. It was not done here for one reason: `mmap` in Rust requires `unsafe`, and
`unsafe_code = "forbid"` is a workspace-wide constraint that buys more than paging would.

The practical consequence: an index has to fit in memory with room to work. Postings compress well —
a few bytes per posting — so tens of millions of documents is reachable on an ordinary machine, but
a corpus that does not fit needs a different engine.

## Terms are keyed by field, not filtered by it

`TermKey` is `(FieldId, String)`, so `title:rust` and `body:rust` are separate posting lists.

The cheaper design is one list per term with a field tag on each posting, filtered at query time.
That breaks scoring: BM25's IDF needs the document frequency *for the field being searched*, and a
shared list only knows the frequency across all fields. A word common in bodies but rare in titles
would be scored as common in both.

The cost is a larger dictionary — the same word appears once per field it occurs in — which
front-coding largely absorbs, since the entries sort adjacently.

## Document ids are dense and segment-local

Ids are assigned `0, 1, 2, …` in insertion order within a segment. That is what makes gap encoding
work: consecutive ids differ by small numbers, so a varint spends one byte where a fixed `u32`
spends four. On a dense posting list this is most of the compression.

It also means an id is meaningless outside its segment. Merging assigns a fresh id space by
offsetting each segment's ids by the count of everything before it, which keeps them dense and
ascending. A caller who needs a stable identifier stores one in a keyword field.

## The query pipeline is three stages, not one

```
text ──lexer──▶ tokens ──parser──▶ AST ──planner──▶ BooleanQuery
```

Each stage fails for a different reason, and a user needs to know which:

- **Lexer** — an unterminated quote, a stray `&`. Reported with a character position.
- **Parser** — unbalanced parentheses, a dangling operator, nesting too deep. Purely syntactic, so
  it needs no index.
- **Planner** — an unknown field, a range on a text field, a phrase on a field without positions.
  These depend on the schema and the dictionary, so they cannot be caught earlier.

Separating them is also what makes the parser testable without an index, which is why there are
76 parser tests that construct no segments at all.

The planner's output is deliberately lossy. `BooleanQuery` is flat — `must`, `must_any`, `should`,
`must_not`, `phrases` — so nested alternation collapses. `(a OR b) AND (c OR d)` becomes two
`must_any` groups, which is exactly right; deeper nesting flattens in ways that change scoring
slightly. [docs/query-language.md](query-language.md) says where.

## `must_any` exists because of a real bug

A bare term with no field searches every default field. Early on, that expansion became one `must`
clause per field, and `rust AND concurrency` returned nothing: it was demanding that *both* words
appear in *both* title and body.

The fix is a clause type the flat boolean did not have — a group of which at least one must match.
`must_any` holds a `Vec<Vec<TermQuery>>`: union within a group, intersect across groups.

Worth recording because the failure mode is silent. A query returning no results looks like an empty
corpus, not like a planner bug, and only a test that indexed a word in one field and queried it
alongside another found it.

## Scoring is behind a trait

`Scorer` takes primitives — term frequency, document frequency, corpus size, field length, average
field length — rather than a struct, because everything it needs is already in registers when the
executor reaches it, and it is called once per matching term per document.

BM25 is the default. TF-IDF is kept for comparison, and `ConstantScorer` exists for filters where
the score is computed and then discarded.

The subtle part is BM25's IDF: `ln(1 + (N - df + 0.5) / (df + 0.5))`. Without the `1 +`, a term
appearing in more than half the documents gets a *negative* weight, so adding a common word to a
query would push matching documents down the ranking. There is a test for exactly that.

## The collector is bounded

`TopK` is a min-heap of size `k`. Sorting every match and truncating would be simpler, but it
allocates proportional to the result set — on a query matching a million documents for the top ten,
that allocation is the whole cost of the query.

The heap also exposes `threshold()`, the worst score currently kept, so an executor can skip
documents that cannot make the cut. The ordering reverses the document-id tiebreak so that once the
heap is inverted for top-k, equal scores come out lowest-id-first and results are stable across
runs.

`ScoredDoc` uses `total_cmp` rather than `partial_cmp`. A pathological boost can produce a NaN
score, and a partial comparison would either panic or quietly break the heap's invariant.

## The segment format is checksummed and framed

```
magic "QRYS" │ version │ body │ CRC32(body) │ magic "QRYE"
```

Both magics matter. A truncated file — the signature of a process killed mid-write — has no trailing
magic, which is a clearer diagnosis than a checksum mismatch. A damaged byte has the footer but
fails the CRC. The two are reported differently because they call for different responses: one means
an interrupted write, the other means damaged storage.

The dictionary is front-coded: terms arrive sorted, so each entry stores the length of the prefix it
shares with the previous term plus its own suffix. `borrow`, `borrowed`, `borrowing` cost six
characters and two short suffixes instead of twenty-one characters.

`docs/format.md` has the byte-level layout.

## The HTTP server uses threads, not async

Once an index is in memory, a request is CPU-bound: walk postings, score, collect. There is no I/O
to overlap, so an async runtime would add a large dependency and a different programming model to
solve a problem this workload does not have. A fixed thread pool sized to the core count is the
right shape.

The pool is fixed rather than thread-per-connection because thread-per-connection makes memory a
function of how many clients show up, which is precisely what an attacker controls.

## What is deliberately missing

- **Deletes and updates.** Needs a tombstone bitmap per segment and a merge that honours it.
- **Range execution.** Numeric fields are indexed by their exact decimal form, so a range needs
  either a post-filter over stored values or a numeric trie. Parsed and planned, not executed.
- **A distributed mode.** Out of scope by a wide margin.
- **Faceting and aggregations.** Would need a column store alongside the inverted index.
- **Non-English analysis.** The pipeline is pluggable; only the English components exist.

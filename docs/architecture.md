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

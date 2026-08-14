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

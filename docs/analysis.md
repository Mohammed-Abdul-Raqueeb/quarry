# Text analysis

How text becomes terms, and where the pipeline is weak. Knowing the weaknesses matters more than
knowing the pipeline: most surprising search results trace back to one of them.

## The pipeline

```
"The Running Cafés"
      │
      ▼  StandardTokenizer
["The", "Running", "Cafés"]
      │
      ▼  Lowercase
["the", "running", "cafés"]
      │
      ▼  AsciiFolding
["the", "running", "cafes"]
      │
      ▼  LengthFilter (1..=64)
["the", "running", "cafes"]
      │
      ▼  StopwordFilter
["running", "cafes"]            position 0 is now empty
      │
      ▼  PorterStemmer
["run", "cafe"]
```

## The one property that matters

**Indexing and querying run the same pipeline.** If a document's `Running` became `run`, then a
query for `Running` must also become `run`, or the document is unfindable.

`Analyzer` is the single object both sides use, which is what stops them drifting. The planner calls
it on query text for exactly this reason, and there is a test asserting
`analyzer.terms("Running") == analyzer.terms("runs")`.

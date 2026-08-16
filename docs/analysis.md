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

## Tokenizers

### StandardTokenizer (the default)

Splits on anything that is not a letter, digit or underscore, with three characters kept *inside* a
token: `'`, `-` and `.`.

Those three are kept because splitting on them produces nonsense: `wasn't` would become `wasn` + `t`,
`e-mail` would become two terms, and `v1.2.3` would become three. They are then trimmed from the
*edges* of a token, because a trailing period is a sentence and a leading dash is a list item.

What it handles: any script that separates words with spaces. `char::is_alphanumeric` covers Latin,
Cyrillic, Greek, Arabic, Hebrew and more.

What it does not handle: Chinese, Japanese and Thai, which do not use spaces. Those produce one
enormous token. Use `NgramTokenizer` instead — this is a real limitation, not an oversight, and a
proper Unicode segmentation implementation would be a large table and a dependency.

### KeywordTokenizer

Emits the whole input as one token. For identifiers, tags and enum values, where splitting would
make an exact lookup impossible. `keyword` schema fields use this.

### WhitespaceTokenizer

Splits on whitespace only, keeping punctuation attached. Rarely what you want for prose; useful when
the input is already tokenized.

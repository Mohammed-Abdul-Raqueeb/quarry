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

### NgramTokenizer and EdgeNgramTokenizer

Character windows. Two jobs:

- Languages without word separators, where bigrams are the standard cheap answer.
- Prefix matching as a plain term lookup, which is how an as-you-type box stays fast without a
  wildcard scan over the dictionary.

The cost is index size: every word becomes several terms. Use it on one field, not everywhere.

## Filters

### Lowercase

Almost always first. Without it `Rust` and `rust` are different terms and a user typing either
expects both.

### AsciiFolding

Strips diacritics, so `café` and `cafe` are the same term. Users type accents inconsistently, often
because their keyboard makes it awkward, so without this half of them cannot find the document.

It covers Latin-1 and Latin Extended-A — the ranges that actually appear in indexed prose — plus the
typographic punctuation that survives tokenization. Anything outside those passes through unchanged
rather than being mangled: leaving text alone is better than corrupting it.

**Order matters.** Folding runs *before* the stopword filter, or `thé` survives a list that contains
`the`.

### LengthFilter

Drops tokens outside 1 to 64 characters. A single character is rarely worth a dictionary entry, and a
300-character "word" is a URL or a hash nobody will search for. Measured in characters, not bytes, so
a four-character accented word is not wrongly dropped.

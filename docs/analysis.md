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

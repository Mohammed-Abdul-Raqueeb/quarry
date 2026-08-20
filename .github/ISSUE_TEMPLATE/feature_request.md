---
name: Feature request
about: Suggest something Quarry should do
labels: enhancement
---

## The problem

What are you trying to search for that you currently cannot?

## What you have tried

Queries or analyzer configurations you attempted, and where they fell short.

## Before you write it up

A few things are deliberately absent, with the reasoning written down. Worth a
look first so the discussion starts from the tradeoff rather than the gap:

- Deletes and updates, distributed mode, faceting, memory-mapped indexes —
  [docs/architecture.md](../../docs/architecture.md)
- Range execution, deeper boolean nesting —
  [docs/query-language.md](../../docs/query-language.md)
- Non-English stemming, CJK tokenization — [docs/analysis.md](../../docs/analysis.md)

If one of those is your request, say what you need it for; a concrete use case
is what would change the decision.

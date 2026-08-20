---
name: Bug report
about: Something behaves differently than documented
labels: bug
---

## What happened

## What you expected

## The smallest reproduction you can manage

Ideally a schema, one or two documents and a query:

```bash
quarry index --index ./idx --schema 'title:text' --input docs.ndjson
quarry search --index ./idx 'your query'
```

If it is a query result you disagree with, `--explain` shows the parsed and
planned form, which is usually where the answer is:

```
quarry search --index ./idx 'your query' --explain
```

## Environment

- Quarry version (`quarry --version`):
- Rust version (`rustc --version`):
- OS:
- Documents in the index (`quarry stats --index ./idx`):

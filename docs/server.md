# HTTP server

```bash
quarry-server --index ./idx [--addr 127.0.0.1:7700] [--threads N] [--quiet]
```

Read-only. Build an index with `quarry index` first.

## Why it is read-only

Indexing holds a write buffer and flushes segments — a lifecycle with memory growth and a commit
point. That does not belong behind an HTTP handler that may be serving a hundred concurrent readers:
the handler would need a lock around the writer, and the flush would stall every reader holding it.

So `quarry index` owns writing and the server owns reading. The split also means the server needs no
mutable state at all, which is what lets it hand the same `AppState` to every thread with no
synchronisation.

Reindexing while the server runs works: build a new index directory, then restart pointing at it.
Hot reload is not implemented.

## Endpoints

### `GET /health`

Liveness. Never touches the index, so a slow query cannot make a health check fail.

```json
{ "status": "ok", "version": "0.4.0", "uptime_seconds": 8241 }
```

### `GET /stats`

```json
{
  "documents": 19043,
  "segments": 3,
  "terms": 48210,
  "bytes": 7245193,
  "fields": 4,
  "bytes_per_doc": 380.5,
  "should_merge": false
}
```

`should_merge` is a hint that the segment count is high enough to be costing read performance.

### `GET /schema`

The field definitions, in the same shape as the `schema.json` inside the index directory.

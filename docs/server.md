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

### `GET /search`

| Parameter | Default | Meaning |
| --- | --- | --- |
| `q` | — | Required. The query |
| `limit` | 10 | Results to return, clamped to 200 |
| `highlight` | off | Also return highlighted snippets |

```bash
curl 'http://127.0.0.1:7700/search?q=rust+AND+ownership&limit=5&highlight'
```

```json
{
  "query": "rust AND ownership",
  "total": 1,
  "took_us": 142,
  "hits": [
    {
      "doc": 0,
      "score": 3.0669,
      "document": { "title": "Rust ownership", "body": "...", "tag": "rust" },
      "highlights": [
        { "field": "title", "snippet": "Rust <mark>ownership</mark>" }
      ]
    }
  ]
}
```

`total` is the number of documents that **matched**, not the number returned. With `limit=1` on a
query matching six documents, `total` is 6 and `hits` has one entry.

A `notes` array appears when the planner had something to say — a dropped stopword, a truncated
expansion, a range that parsed but was not executed. Without it, a result set shaped by one of those
looks arbitrary.

### `POST /search`

The same thing with a JSON body, for queries awkward to URL-encode:

```bash
curl -X POST http://127.0.0.1:7700/search \
  -d '{"q":"title:\"zero cost\" AND NOT deprecated","limit":5}'
```

## Status codes

| Code | When |
| --- | --- |
| 200 | Success, including a search that matched nothing |
| 400 | Malformed request, or a query the parser rejected |
| 404 | No such endpoint |
| 405 | Known endpoint, wrong method |
| 413 | Request body over 16 MB |
| 422 | Well-formed but unprocessable: no `q`, or a query the planner rejected |
| 500 | The engine failed |

The 400/422 split follows the stage that rejected the query. A syntax error is 400; an unknown field
or a phrase on a positionless field is 422, because the request parsed fine and the problem is
semantic. An unknown field also gets a spelling suggestion:

```json
{ "error": "no field named \"titel\"; did you mean \"title\"?" }
```

Every response body is JSON, including every error. There is a test asserting that across every
endpoint and method combination.

## Concurrency

A fixed thread pool, sized to the core count and clamped to 2–32, or set with `--threads`.

Fixed rather than thread-per-connection because thread-per-connection makes memory a function of how
many clients connect, which is exactly what an attacker controls. Requests queue instead.

No async runtime. Once the index is in memory a request is CPU-bound — walk postings, score, collect
— so there is no I/O to overlap and threads equal to cores is the right shape. An executor would add
a large dependency to solve a problem this workload does not have.

A panicking handler kills its job, not the pool: the worker is replaced on the next loop iteration
and the server keeps serving. There is a test for that.

## Limits

| Limit | Value |
| --- | --- |
| Request line | 8 KB |
| Headers | 64, each up to 8 KB |
| Body | 16 MB |
| Requests per kept-alive connection | 100 |
| Idle timeout | 30 seconds |
| `limit` parameter | 200 |

The idle timeout and the per-connection request cap exist for the same reason: without them an idle
client holds a pool worker indefinitely, and enough of them starve the server without sending a
single request.

The `limit` clamp matters more than it looks. `?limit=100000000` would otherwise ask the collector
for a heap that size before a single document is scored.

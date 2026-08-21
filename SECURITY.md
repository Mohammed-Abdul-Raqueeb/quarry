# Security policy

## Reporting

Please do not open a public issue for a security problem. Use GitHub's private vulnerability
reporting on this repository, and include what an attacker can do, how to reproduce it, and the
version you tested.

Expect an acknowledgement within a few days.

## What the attack surface actually is

Quarry parses untrusted input in four places. These are where a vulnerability would be:

| Component | Input | Guards |
| --- | --- | --- |
| `json.rs` | Documents, request bodies | Depth capped at 64; no recursion without it |
| `query/lexer.rs` | Query text | 4096-byte length cap, 24-level nesting cap |
| `index/segment.rs` | Segment files | CRC32, framing magics, every length bounded and checked |
| `server/http.rs` | HTTP requests | Request line, header and body limits; idle timeout |

`unsafe` is forbidden workspace-wide and there are no dependencies, so the usual sources of memory
unsafety and supply-chain risk are structurally absent rather than merely audited.

## Specific hardening worth knowing about

- **No allocation from an unvalidated length.** A count read from a segment is clamped before it
  reaches `Vec::with_capacity`, so a corrupt four-byte field cannot request gigabytes before the
  first read fails.
- **Varints reject redundant padding**, so two byte sequences cannot encode the same value.
- **Front-coded prefixes are checked** against the previous term's length and for UTF-8 character
  boundaries.
- **Every arithmetic step on an on-disk value is checked** for overflow.
- **The query planner never interpolates a column name from input.** Field names resolve through the
  schema to a numeric id; an unknown name is an error.
- **The server's `limit` is clamped to 200**, so `?limit=100000000` cannot ask the collector for a
  heap that size before any document is scored.
- **The thread pool is fixed**, so connection count does not drive memory.

## Known limitations

These are design positions, not oversights:

- **The server has no authentication and no TLS.** Anything reachable should be internal or behind a
  reverse proxy. See [docs/server.md](docs/server.md).
- **An index is loaded wholly into memory.** A hostile index file could therefore demand a lot of
  memory. Only index files you trust — they are a database, not a document format.
- **Stored field values are kept verbatim.** Whatever you index is readable by anyone who can query
  it or read the segment file. Do not index secrets.
- **No query timeout.** A pathological query — a phrase of common terms over a very large corpus —
  will run to completion. Put a timeout at the proxy.

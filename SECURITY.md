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

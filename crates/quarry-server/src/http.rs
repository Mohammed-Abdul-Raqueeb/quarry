//! Just enough HTTP/1.1 to serve a search API.
//!
//! Not a general-purpose server. It handles the subset Quarry's API needs —
//! `GET` and `POST`, a `Content-Length` body, query strings, keep-alive — and
//! deliberately omits chunked request bodies, pipelining, TLS and HTTP/2. The
//! deployment note is the honest one: put this behind a reverse proxy, which is
//! where those belong anyway.
//!
//! What it does take seriously is the hostile-input surface, because that is
//! what a network-facing parser is for: every length is bounded, the request
//! line and headers are capped, and a body larger than the limit is rejected
//! before it is read.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;

/// Longest request line accepted, in bytes.
pub const MAX_REQUEST_LINE: usize = 8 * 1024;
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
/// Most headers accepted.
pub const MAX_HEADERS: usize = 64;
/// Longest single header line, in bytes.
pub const MAX_HEADER_LINE: usize = 8 * 1024;
/// Largest request body accepted, in bytes.
pub const MAX_BODY: usize = 16 * 1024 * 1024;

/// A parsed request.
#[derive(Debug, Clone)]
pub struct Request {
    /// The method, uppercased.
    pub method: String,
    /// The path, with the query string removed and percent-escapes decoded.
    pub path: String,
    /// Decoded query parameters.
    pub query: HashMap<String, String>,
    /// Headers, with lowercased names.
    pub headers: HashMap<String, String>,
    /// The body, empty when there was none.
    pub body: String,
}

impl Request {
    /// A query parameter.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(String::as_str)
    }

    /// A query parameter, or a default.
    #[must_use]
    pub fn param_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.param(name).unwrap_or(default)
    }

    /// A query parameter parsed as a number.
    #[must_use]
    pub fn param_num(&self, name: &str, default: usize) -> usize {
        self.param(name)
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(default)
    }

    /// Whether a boolean-ish parameter is set.
    ///
    /// `?pretty`, `?pretty=1` and `?pretty=true` all count, which is what
    /// someone poking at the API with curl will expect.
    #[must_use]
    pub fn param_flag(&self, name: &str) -> bool {
        matches!(
            self.query.get(name).map(String::as_str),
            Some("" | "1" | "true" | "yes")
        )
    }

    /// A header, by lowercase name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// Whether the client asked to keep the connection open.
    ///
    /// HTTP/1.1 defaults to keep-alive, so only an explicit `close` ends it.
    #[must_use]
    pub fn wants_keep_alive(&self) -> bool {
        !self
            .header("connection")
            .is_some_and(|value| value.eq_ignore_ascii_case("close"))
    }
}

/// A response to write back.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// `Content-Type` value.
    pub content_type: String,
    /// The body.
    pub body: String,
    /// Extra headers.
    pub headers: Vec<(String, String)>,
}

impl Response {
    /// A JSON response.
    #[must_use]
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8".to_string(),
            body: body.into(),
            headers: Vec::new(),
        }
    }

    /// A plain-text response.
    #[must_use]
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: body.into(),
            headers: Vec::new(),
        }
    }

    /// A JSON error body, in the one shape every endpoint uses.
    #[must_use]
    pub fn error(status: u16, message: &str) -> Self {
        let escaped = escape_json_string(message);
        Self::json(status, format!("{{\"error\":\"{escaped}\"}}"))
    }

    /// Adds a header.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Serializes the response.
    #[must_use]
    pub fn render(&self, keep_alive: bool) -> Vec<u8> {
        let reason = reason_phrase(self.status);
        let mut out = format!(
            "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: {}\r\n",
            self.status,
            self.content_type,
            self.body.len(),
            if keep_alive { "keep-alive" } else { "close" }
        );

        for (name, value) in &self.headers {
            // Written in place rather than through a temporary String, since
            // the header block can hold a handful of entries per response.
            let _ = write!(out, "{name}: {value}\r\n");
        }

        out.push_str("\r\n");

        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(self.body.as_bytes());
        bytes
    }
}

/// Why a request could not be read.
#[derive(Debug)]
pub enum HttpError {
    /// The socket failed.
    Io(std::io::Error),
    /// The request was syntactically wrong.
    Malformed(String),
    /// A declared length exceeded a limit.
    TooLarge(String),
    /// The peer closed the connection without sending anything.
    Closed,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Malformed(what) => write!(f, "malformed request: {what}"),
            Self::TooLarge(what) => write!(f, "request too large: {what}"),
            Self::Closed => f.write_str("connection closed"),
        }
    }
}

impl From<std::io::Error> for HttpError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Reads one request from a connection.
pub fn read_request(stream: &mut BufReader<&TcpStream>) -> Result<Request, HttpError> {
    let mut line = String::new();
    let read = read_line_limited(stream, &mut line, MAX_REQUEST_LINE)?;

    if read == 0 {
        return Err(HttpError::Closed);
    }

    let mut parts = line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("empty request line".to_string()))?
        .to_uppercase();
    let target = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("no request target".to_string()))?;

    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));

    let mut headers = HashMap::new();
    for _ in 0..MAX_HEADERS {
        let mut header_line = String::new();
        let read = read_line_limited(stream, &mut header_line, MAX_HEADER_LINE)?;

        if read == 0 {
            return Err(HttpError::Malformed(
                "headers were not terminated".to_string(),
            ));
        }

        let trimmed = header_line.trim_end();
        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
        // A header line with no colon is skipped rather than fatal: it costs
        // nothing to ignore and some clients send odd things.
    }

    let body = read_body(stream, &headers)?;

    Ok(Request {
        method,
        path: percent_decode(raw_path),
        query: parse_query(raw_query),
        headers,
        body,
    })
}

/// Reads a body according to `Content-Length`.
fn read_body(
    stream: &mut BufReader<&TcpStream>,
    headers: &HashMap<String, String>,
) -> Result<String, HttpError> {
    // Chunked encoding is not supported; saying so beats hanging while waiting
    // for a body that will never arrive in the expected form.
    if headers
        .get("transfer-encoding")
        .is_some_and(|value| value.to_lowercase().contains("chunked"))
    {
        return Err(HttpError::Malformed(
            "chunked request bodies are not supported".to_string(),
        ));
    }

    let Some(raw_length) = headers.get("content-length") else {
        return Ok(String::new());
    };

    let length: usize = raw_length
        .parse()
        .map_err(|_| HttpError::Malformed(format!("bad content-length: {raw_length}")))?;

    // Checked before allocating, so a header claiming four gigabytes costs
    // nothing.
    if length > MAX_BODY {
        return Err(HttpError::TooLarge(format!(
            "body of {length} bytes exceeds the {MAX_BODY} byte limit"
        )));
    }

    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer)?;

    String::from_utf8(buffer).map_err(|_| HttpError::Malformed("body is not utf-8".to_string()))
}

/// Reads a line, refusing to grow past `limit`.
fn read_line_limited(
    stream: &mut BufReader<&TcpStream>,
    out: &mut String,
    limit: usize,
) -> Result<usize, HttpError> {
    let mut total = 0;

    loop {
        let mut byte = [0u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => return Ok(total),
            Ok(_) => {}
            Err(err) => return Err(HttpError::Io(err)),
        }

        total += 1;
        if total > limit {
            return Err(HttpError::TooLarge(format!("line exceeds {limit} bytes")));
        }

        let ch = byte[0] as char;
        out.push(ch);
        if ch == '\n' {
            return Ok(total);
        }
    }
}

/// Parses a query string into decoded pairs.
#[must_use]
pub fn parse_query(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();

    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(percent_decode(key), percent_decode(value));
    }

    out
}

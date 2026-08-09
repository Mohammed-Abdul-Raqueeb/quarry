//! `quarry-server` — serve a Quarry index over HTTP.
//!
//! ```text
//! quarry-server --index DIR [--addr 127.0.0.1:7700] [--threads N] [--quiet]
//! ```

use quarry_core::tokenize::Analyzer;
use quarry_core::IndexDir;
use quarry_server::http::{read_request, write_response, HttpError, Response};
use quarry_server::routes::{access_log, route, AppState};
use quarry_server::ThreadPool;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a kept-alive connection may sit idle before it is closed.
///
/// Without this, an idle client holds a pool worker indefinitely, and enough of
/// them starve the server without sending a single request.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Most requests served on one kept-alive connection before it is closed.
const MAX_REQUESTS_PER_CONNECTION: usize = 100;

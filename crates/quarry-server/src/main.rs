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

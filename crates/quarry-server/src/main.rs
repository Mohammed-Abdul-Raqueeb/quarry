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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("quarry-server {}", quarry_server::VERSION);
        return ExitCode::SUCCESS;
    }

    let Some(index_path) = flag_value(&args, "--index", "-i") else {
        eprintln!("error: --index is required");
        print_help();
        return ExitCode::FAILURE;
    };

    let addr = flag_value(&args, "--addr", "-a").unwrap_or_else(|| "127.0.0.1:7700".to_string());
    let threads = flag_value(&args, "--threads", "-t").and_then(|raw| raw.parse::<usize>().ok());
    let quiet = args.iter().any(|a| a == "--quiet" || a == "-q");

    match serve(&index_path, &addr, threads, quiet) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn serve(
    index_path: &str,
    addr: &str,
    threads: Option<usize>,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let index = IndexDir::open(index_path)?;

    let state = Arc::new(AppState {
        analyzer: Analyzer::english(),
        started: Instant::now(),
        index,
    });

    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;

    let pool = match threads {
        Some(n) => ThreadPool::new(n),
        None => ThreadPool::sized_for_machine(),
    };

    eprintln!(
        "quarry-server {} serving {} documents from {} on http://{} ({} threads)",
        quarry_server::VERSION,
        state.index.doc_count(),
        index_path,
        local,
        pool.size()
    );

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(err) => {
                // One failed accept is not a reason to stop serving.
                eprintln!("accept failed: {err}");
                continue;
            }
        };

        let state = Arc::clone(&state);

        if let Err(reason) = pool.execute(move || handle_connection(stream, &state, quiet)) {
            eprintln!("rejecting connection: {reason}");
            break;
        }
    }

    Ok(())
}

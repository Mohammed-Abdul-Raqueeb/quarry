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

fn handle_connection(mut stream: TcpStream, state: &AppState, quiet: bool) {
    if stream.set_read_timeout(Some(IDLE_TIMEOUT)).is_err() {
        return;
    }
    // Nagle's algorithm delays a small response waiting for more to send, which
    // is exactly wrong for a request/response API.
    let _ = stream.set_nodelay(true);

    for _ in 0..MAX_REQUESTS_PER_CONNECTION {
        let mut reader = BufReader::new(&stream);

        let request = match read_request(&mut reader) {
            Ok(request) => request,
            // A closed connection is the normal way a keep-alive loop ends.
            Err(HttpError::Closed) => return,
            Err(err) => {
                let status = match err {
                    HttpError::TooLarge(_) => 413,
                    _ => 400,
                };
                let response = Response::error(status, &err.to_string());
                let _ = write_response(&mut stream, &response, false);
                return;
            }
        };

        let started = Instant::now();
        let response = route(&request, state);
        let elapsed = started.elapsed();

        if !quiet {
            eprintln!("{}", access_log(&request, &response, elapsed));
        }

        let keep_alive = request.wants_keep_alive();

        if write_response(&mut stream, &response, keep_alive).is_err() || !keep_alive {
            return;
        }
    }
}

fn flag_value(args: &[String], long: &str, short: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if let Some(value) = arg.strip_prefix(&format!("{long}=")) {
            return Some(value.to_string());
        }
        if arg == long || arg == short {
            return args.get(i + 1).cloned();
        }

        i += 1;
    }
    None
}

fn print_help() {
    println!(
        "\
quarry-server {version} — serve a Quarry index over HTTP

USAGE
    quarry-server --index DIR [options]

OPTIONS
    -i, --index DIR      Index directory to serve. Required
    -a, --addr ADDR      Listen address. Default: 127.0.0.1:7700
    -t, --threads N      Worker threads. Default: one per core, 2 to 32
    -q, --quiet          Do not write an access log
    -h, --help           Show this help
    -V, --version        Show the version

ENDPOINTS
    GET  /health                     liveness; never touches the index
    GET  /stats                      documents, segments, terms, bytes
    GET  /schema                     the field definitions
    GET  /search?q=QUERY&limit=N     run a query
                 &highlight          also return highlighted snippets
    POST /search                     {{\"q\": \"...\", \"limit\": 10}}

NOTES
    Read-only. Build an index with `quarry index` first.
    No TLS: run it behind a reverse proxy if it faces the internet.

EXAMPLES
    quarry-server --index ./idx
    curl 'http://127.0.0.1:7700/search?q=rust+ownership&limit=5&highlight'
    curl -X POST http://127.0.0.1:7700/search -d '{{\"q\":\"title:rust\"}}'
",
        version = quarry_server::VERSION
    );
}

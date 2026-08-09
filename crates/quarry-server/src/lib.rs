//! A dependency-free HTTP server over a Quarry index.
//!
//! `std::net` plus a fixed thread pool, with just enough HTTP/1.1 to serve a
//! JSON search API. There is no async runtime because there is no need for one:
//! once the index is in memory a request is CPU-bound, so threads equal to
//! cores is the right shape and an executor would only add a dependency.
//!
//! Read-only by design. See [`routes`] for why indexing stays in the CLI.

pub mod http;
pub mod pool;
pub mod routes;

pub use http::{Request, Response};
pub use pool::ThreadPool;
pub use routes::{route, AppState};

/// The version of this crate, from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

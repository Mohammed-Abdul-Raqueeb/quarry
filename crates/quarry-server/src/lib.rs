//! A dependency-free HTTP server over a Quarry index.
//!
//! `std::net` plus a fixed thread pool, with just enough HTTP/1.1 to serve a
//! JSON search API. There is no async runtime because there is no need for one:
//! once the index is in memory a request is CPU-bound, so threads equal to
//! cores is the right shape and an executor would only add a dependency.
//!
//! Read-only by design. See [`routes`] for why indexing stays in the CLI.

//! The Quarry command line interface, as a library.
//!
//! The binary is a thin wrapper over [`run`]. Splitting them is what makes the
//! argument parser and the output helpers testable from an integration test
//! rather than only through a subprocess.

pub mod args;
pub mod commands;
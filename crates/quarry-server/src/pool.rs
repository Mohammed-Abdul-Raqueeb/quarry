//! A fixed thread pool.
//!
//! Spawning a thread per connection is the obvious approach and the wrong one:
//! it makes the server's memory a function of how many clients show up, which
//! is exactly the thing an attacker controls. A fixed pool bounds concurrency
//! at a number chosen at startup, and queues the rest.
//!
//! Built on `std::sync::mpsc` and `Mutex`, because that is all a pool needs.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// A unit of work.
type Job = Box<dyn FnOnce() + Send + 'static>;

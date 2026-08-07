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

/// What the workers receive.
enum Message {
    /// Run this.
    Run(Job),
    /// Stop, so the thread can be joined.
    ///
    /// Dropping the sender would also end the loop, but an explicit message
    /// lets `shutdown` send exactly one per worker and know they all got one.
    Stop,
}

/// A fixed set of worker threads pulling from a shared queue.
pub struct ThreadPool {
    sender: Option<Sender<Message>>,
    workers: Vec<Worker>,
}

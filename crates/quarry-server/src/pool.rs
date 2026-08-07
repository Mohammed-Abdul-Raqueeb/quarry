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

struct Worker {
    id: usize,
    handle: Option<JoinHandle<()>>,
}

impl ThreadPool {
    /// A pool with `size` threads.
    ///
    /// `size` is clamped to at least one: a zero-sized pool would accept jobs
    /// and never run them, which looks exactly like a hang.
    #[must_use]
    pub fn new(size: usize) -> Self {
        let size = size.max(1);
        let (sender, receiver) = channel::<Message>();

        // One receiver shared behind a mutex: whichever worker acquires the
        // lock first takes the next job, which is the queue discipline wanted.
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::spawn(id, Arc::clone(&receiver)));
        }

        Self {
            sender: Some(sender),
            workers,
        }
    }

    /// A pool sized to the machine, within reason.
    #[must_use]
    pub fn sized_for_machine() -> Self {
        let parallelism = thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);

        // Requests are CPU-bound once the index is in memory, so there is
        // nothing to gain from more threads than cores. The cap keeps a
        // 128-core host from allocating stacks it will never use.
        Self::new(parallelism.clamp(2, 32))
    }

    /// How many workers there are.
    #[must_use]
    pub fn size(&self) -> usize {
        self.workers.len()
    }

    /// Queues a job.
    ///
    /// Returns `Err` only once the pool is shutting down, which a caller can
    /// treat as "stop accepting connections".
    pub fn execute<F>(&self, job: F) -> Result<(), &'static str>
    where
        F: FnOnce() + Send + 'static,
    {
        let sender = self.sender.as_ref().ok_or("thread pool is shut down")?;
        sender
            .send(Message::Run(Box::new(job)))
            .map_err(|_| "thread pool workers have all stopped")
    }

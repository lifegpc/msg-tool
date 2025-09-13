//! Thread pool utilities
use crate::ext::mutex::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{
    Arc, Condvar, Mutex,
    mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
};
use std::thread::{self, JoinHandle};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

/// A simple generic thread pool.
///
/// - T: the return type of tasks. Completed task results are stored in `results: Arc<Mutex<Vec<T>>>`.
/// - execute accepts a task and a `block_if_full` flag:
///     * if true, submission will block when the pool is saturated until a worker becomes available;
///     * if false, submission will return an error when the pool is saturated.
/// - join waits until all submitted tasks have completed (it does not shut down the pool).
pub struct ThreadPool<T: Send + 'static> {
    sender: Option<SyncSender<Job<T>>>,
    #[allow(unused)]
    receiver: Arc<Mutex<Receiver<Job<T>>>>,
    workers: Vec<JoinHandle<()>>,
    /// Completed task results
    pub results: Arc<Mutex<Vec<T>>>,
    /// Number of pending tasks (queued + running)
    pending: Arc<AtomicUsize>,
    /// Pair for wait/notify in join
    pending_pair: Arc<(Mutex<()>, Condvar)>,
    size: usize,
}

#[derive(Debug)]
/// Error type for [ThreadPool::execute]
pub enum ExecuteError {
    /// Pool is full
    Full,
    /// Pool is closed
    Closed,
}

impl std::error::Error for ExecuteError {}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecuteError::Full => write!(f, "ThreadPool is full"),
            ExecuteError::Closed => write!(f, "ThreadPool is closed"),
        }
    }
}

impl<T: Send + 'static> ThreadPool<T> {
    /// Get the number of worker threads in the pool.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Create a new thread pool with `size` workers.
    /// The internal submission channel is bounded to `size`, so when all workers are busy and
    /// the channel is full, further submissions will block or return error depending on the flag.
    ///
    /// * `name` - Optional base name for worker threads. If None, "threadpool-worker-" is used.
    pub fn new<'a>(size: usize, name: Option<&'a str>) -> Result<Self, std::io::Error> {
        if size == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "worker size must be > 0",
            ));
        }

        let (tx, rx) = sync_channel::<Job<T>>(size);
        let receiver = Arc::new(Mutex::new(rx));
        let results = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(AtomicUsize::new(0));
        let pending_pair = Arc::new((Mutex::new(()), Condvar::new()));
        let thread_name = name.unwrap_or("threadpool-worker-");

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            let rx_clone = Arc::clone(&receiver);
            let results_clone = Arc::clone(&results);
            let pending_clone = Arc::clone(&pending);
            let pending_pair_clone = Arc::clone(&pending_pair);

            let handle = thread::Builder::new()
                .name(format!("{}{}", thread_name, id))
                .spawn(move || {
                    loop {
                        // Lock receiver to call recv. Using a Mutex around Receiver serializes
                        // the recv calls but is fine for this simple implementation.
                        let job = {
                            let guard = rx_clone.lock_blocking();
                            // If recv returns Err, sender was dropped -> exit thread
                            guard.recv()
                        };

                        match job {
                            Ok(job) => {
                                // Execute the job and store result
                                let res = job();
                                {
                                    let mut r = results_clone.lock_blocking();
                                    r.push(res);
                                }

                                // Decrement pending count and notify join waiters
                                pending_clone.fetch_sub(1, Ordering::SeqCst);
                                let (lock, cvar) = &*pending_pair_clone;
                                let _g = lock.lock_blocking();
                                cvar.notify_all();
                            }
                            Err(_) => {
                                // Channel closed -> shutdown worker
                                break;
                            }
                        }
                    }
                })?;

            workers.push(handle);
        }

        Ok(ThreadPool {
            sender: Some(tx),
            receiver,
            workers,
            results,
            pending,
            pending_pair,
            size,
        })
    }

    /// Execute a task. If `block_if_full` is true, this call will block when the internal
    /// submission channel is full (i.e. all workers busy and buffer full) until space becomes available.
    /// If `block_if_full` is false, this returns Err(ExecuteError::Full) when the channel is full.
    pub fn execute<F>(&self, job: F, block_if_full: bool) -> Result<(), ExecuteError>
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let sender = match &self.sender {
            Some(s) => s,
            None => return Err(ExecuteError::Closed),
        };

        // Increase pending count for this submission. If submission fails we will decrement.
        self.pending.fetch_add(1, Ordering::SeqCst);

        let boxed: Job<T> = Box::new(job);

        if block_if_full {
            // This will block until there is space in the bounded channel or the channel is closed.
            if sender.send(boxed).is_err() {
                // Channel closed
                self.pending.fetch_sub(1, Ordering::SeqCst);
                return Err(ExecuteError::Closed);
            }
            Ok(())
        } else {
            match sender.try_send(boxed) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => {
                    // revert pending increment
                    self.pending.fetch_sub(1, Ordering::SeqCst);
                    Err(ExecuteError::Full)
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.pending.fetch_sub(1, Ordering::SeqCst);
                    Err(ExecuteError::Closed)
                }
            }
        }
    }

    /// Wait until all submitted tasks have completed. This does not shut down the pool; new tasks
    /// can still be submitted after join returns.
    pub fn join(&self) {
        // Fast path
        if self.pending.load(Ordering::SeqCst) == 0 {
            return;
        }

        let (lock, cvar) = &*self.pending_pair;
        let mut guard = lock.lock_blocking();
        while self.pending.load(Ordering::SeqCst) != 0 {
            guard = match cvar.wait(guard) {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }

    /// Wait until all submitted tasks have completed, then return the results.
    pub fn into_results(self) -> Vec<T> {
        self.join();
        let mut results = self.results.lock_blocking();
        results.split_off(0)
    }
}

impl<T: Send + 'static> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        // Close sender so worker threads exit recv loop
        self.sender.take();
        // Dropping the sender (SyncSender) happens above; but to ensure we close the channel we
        // explicitly drop any remaining clones by letting sender go out of scope.

        // Join worker threads
        while let Some(handle) = self.workers.pop() {
            let _ = handle.join();
        }
    }
}

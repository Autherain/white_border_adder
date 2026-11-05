//! A bounded pool with results handed back as they complete.

use std::sync::mpsc;
use std::sync::{Mutex, PoisonError};
use std::thread;

/// Calls `f` on every item from `workers` threads and hands each result to
/// `done` on the calling thread as soon as it is ready, in completion order,
/// so the caller can print progress while the rest is still running. Items
/// are moved into the workers, never copied.
pub fn run_each<T: Send, R: Send>(
    workers: usize,
    items: Vec<T>,
    f: impl Fn(T) -> R + Sync,
    mut done: impl FnMut(R),
) {
    let workers = workers.clamp(1, items.len().max(1));
    let queue = Mutex::new(items.into_iter());
    let (tx, rx) = mpsc::channel();

    thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let queue = &queue;
            let f = &f;
            scope.spawn(move || {
                loop {
                    // Taken on its own line: a guard born inside a `while let`
                    // condition lives for the whole body, and the pool would
                    // run one item at a time with the lock held during `f`.
                    let next = queue.lock().unwrap_or_else(PoisonError::into_inner).next();
                    let Some(item) = next else { break };
                    if tx.send(f(item)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(tx);

        for r in rx {
            done(r);
        }
    });
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

//! A bounded pool with results handed back as they complete.

use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
use std::thread;

/// Calls `work` on every item from `workers` threads and hands each result to
/// `done` on the calling thread as soon as it is ready, in completion order,
/// so the caller can print progress while the rest is still running. Items
/// are moved into the workers, never copied.
pub fn run_each<T: Send, R: Send>(
    workers: usize,
    items: Vec<T>,
    work: impl Fn(T) -> R + Sync,
    mut done: impl FnMut(R),
) {
    // Never more threads than items, never zero.
    let mut threads = workers;
    if threads > items.len() {
        threads = items.len();
    }
    if threads == 0 {
        threads = 1;
    }
    let queue = Mutex::new(items.into_iter());
    let (tx, rx) = mpsc::channel();

    thread::scope(|scope| {
        for _ in 0..threads {
            let tx = tx.clone();
            let queue = &queue;
            let work = &work;
            scope.spawn(move || {
                loop {
                    // Taken on its own line: a guard born inside a `while let`
                    // condition lives for the whole body, and the pool would
                    // run one item at a time with the lock held during `work`.
                    let next = locked(queue).next();
                    let item = match next {
                        Some(i) => i,
                        None => break,
                    };
                    // The receiver is gone only when the caller panicked;
                    // nobody is left to hand a result to.
                    if tx.send(work(item)).is_err() {
                        return;
                    }
                }
            });
        }
        // Each worker holds a clone; the loop below ends when the last
        // sender is dropped, so this one must not outlive the workers.
        drop(tx);

        for result in rx {
            done(result);
        }
    });
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

/// A worker that panicked while holding the lock poisons it; the queue it
/// guards is still sound, so the pool goes on.
fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

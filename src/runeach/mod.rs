//! A bounded pool with results handed back as they complete.

use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
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
                    let next = locked(queue).next();
                    let item = match next {
                        Some(i) => i,
                        None => break,
                    };
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

/// A worker that panicked while holding the lock poisons it; the queue it
/// guards is still sound, so the pool goes on.
fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

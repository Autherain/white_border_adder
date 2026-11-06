#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn reports_every_result() {
    let mut got = Vec::new();
    run_each(4, vec![1, 2, 3, 4, 5], |n| n * n, |r| got.push(r));
    got.sort_unstable();
    assert_eq!(got, [1, 4, 9, 16, 25]);
}

#[test]
fn done_runs_on_the_calling_thread() {
    let caller = thread::current().id();
    run_each(
        2,
        vec![(), ()],
        |()| thread::current().id(),
        |worker| {
            assert_ne!(worker, caller);
            assert_eq!(thread::current().id(), caller);
        },
    );
}

#[test]
fn items_are_moved_not_copied() {
    let items = vec![String::from("a"), String::from("b")];
    let mut got = Vec::new();
    run_each(0, items, |s| s + "!", |r| got.push(r));
    got.sort();
    assert_eq!(got, ["a!", "b!"]);
}

#[test]
fn nothing_to_do_returns_at_once() {
    run_each::<(), ()>(8, Vec::new(), |()| (), |()| panic!("done called"));
}

#[test]
fn workers_run_at_the_same_time() {
    use std::time::{Duration, Instant};

    let started = Instant::now();
    run_each(
        4,
        vec![(); 4],
        |()| thread::sleep(Duration::from_millis(100)),
        |()| {},
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(300),
        "4 x 100 ms took {elapsed:?}: the pool is serialised"
    );
}

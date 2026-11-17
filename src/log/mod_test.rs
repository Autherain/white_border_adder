#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::testfs::Buffer;

#[test]
fn levels() {
    for verbose in [false, true] {
        let out = Buffer::default();
        let err = Buffer::default();
        let log = Lines::new(Box::new(out.clone()), Box::new(err.clone()), verbose);
        log.debug("skip a");
        log.info("would delete b");
        log.error("c: boom");

        let want = if verbose {
            "skip a\nwould delete b\n"
        } else {
            "would delete b\n"
        };
        assert_eq!(out.text(), want, "verbose={verbose}");
        assert_eq!(err.text(), "####### c: boom\n");
    }
}

#[test]
fn silent_says_nothing() {
    Silent.info("lost");
}

#[test]
fn mock_checks_the_exact_line() {
    let mut log = MockLog::new();
    log.expect_info()
        .with(mockall::predicate::eq("hello"))
        .times(1)
        .return_const(());
    log.info("hello");
}

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn levels() {
    for verbose in [false, true] {
        let log = Lines::new(Vec::new(), Vec::new(), verbose);
        log.debug("skip a");
        log.info("would delete b");
        log.error("c: boom");

        let (out, err) = log.into_parts();
        let want = if verbose {
            "skip a\nwould delete b\n"
        } else {
            "would delete b\n"
        };
        assert_eq!(String::from_utf8_lossy(&out), want, "verbose={verbose}");
        assert_eq!(String::from_utf8_lossy(&err), "####### c: boom\n");
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

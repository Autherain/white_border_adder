#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn plain_when_not_a_terminal() {
    assert_eq!(Palette::new(false).would("would delete"), "would delete");
    assert_ne!(Palette::new(true).would("would delete"), "would delete");
}

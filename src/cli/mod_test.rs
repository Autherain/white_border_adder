#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::testfs::Buffer;

#[test]
fn human_rounds_like_a_photographer() {
    let cases = [
        (5, "5 B"),
        (1_500, "2 kB"),
        (1_000_000, "1.0 MB"),
        (24_012_000, "24 MB"),
        (2_400_000_000, "2.4 GB"),
    ];
    for (bytes, want) in cases {
        assert_eq!(human(bytes), want);
    }
}

#[test]
fn cli_parses_every_verb() {
    Cli::command().debug_assert();
}

#[test]
fn version_and_bad_flag() {
    let locate: Locate = Box::new(|_| anyhow::bail!("unused"));
    let out = Buffer::default();
    let code = run(
        locate,
        ["wba", "version"].map(OsString::from),
        out.clone(),
        Buffer::default(),
        false,
    );
    assert_eq!(code, ExitCode::SUCCESS);
    assert_eq!(out.text(), format!("{VERSION}\n"));

    let locate: Locate = Box::new(|_| anyhow::bail!("unused"));
    let err = Buffer::default();
    let code = run(
        locate,
        ["wba", "clean", "--nope", "d"].map(OsString::from),
        Buffer::default(),
        err.clone(),
        false,
    );
    assert_ne!(code, ExitCode::SUCCESS);
    assert!(err.text().contains("--nope"));
}

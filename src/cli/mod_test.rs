#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

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
    let mut out = Vec::new();
    let code = run(
        locate,
        ["wba", "version"].map(OsString::from),
        &mut out,
        Vec::new(),
        false,
    );
    assert_eq!(code, ExitCode::SUCCESS);
    assert_eq!(String::from_utf8_lossy(&out), format!("{VERSION}\n"));

    let locate: Locate = Box::new(|_| anyhow::bail!("unused"));
    let mut err = Vec::new();
    let code = run(
        locate,
        ["wba", "clean", "--nope", "d"].map(OsString::from),
        Vec::new(),
        &mut err,
        false,
    );
    assert_ne!(code, ExitCode::SUCCESS);
    assert!(String::from_utf8_lossy(&err).contains("--nope"));
}

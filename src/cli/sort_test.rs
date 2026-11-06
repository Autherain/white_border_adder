#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::SystemTime;

use crate::testfs::new_tree;

use crate::cli::show::tests::{output, test_app};

use super::*;

#[test]
fn sort_dry_run_then_apply() {
    for apply in [false, true] {
        let root = new_tree(&[
            ("r/d/a.png", b"x"),
            ("r/d/a.png.pp3", b"y"),
            ("r/2026/20260620_toulouse/b.png", b"z"),
        ]);
        let noon = jiff::civil::date(2026, 6, 20)
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::system())
            .unwrap();
        for p in [
            "r/d/a.png",
            "r/d/a.png.pp3",
            "r/2026/20260620_toulouse/b.png",
        ] {
            root.join(p)
                .and_then(|f| f.set_modification_time(SystemTime::from(noon.timestamp())))
                .unwrap();
        }

        let at = root.clone();
        let app = test_app(apply, Box::new(move |p| Ok(at.join(p)?)));
        app.sort(SortArgs {
            dir: "r".to_owned(),
            sources: vec!["r/d".to_owned()],
            exclude: vec![],
        })
        .unwrap();

        let want = if apply {
            "moved d/a.png -> 2026/20260620_toulouse/a.png\n\
             moved d/a.png.pp3 -> 2026/20260620_toulouse/a.png.pp3\n\
             2 files moved.\n"
        } else {
            "would move d/a.png -> 2026/20260620_toulouse/a.png\n\
             would move d/a.png.pp3 -> 2026/20260620_toulouse/a.png.pp3\n\
             2 files. Re-run with --apply to move them.\n"
        };
        assert_eq!(output(app), want, "apply={apply}");

        let moved = root
            .join("r/2026/20260620_toulouse/a.png")
            .and_then(|p| p.exists())
            .unwrap_or(false);
        assert_eq!(moved, apply, "apply={apply}");
    }
}

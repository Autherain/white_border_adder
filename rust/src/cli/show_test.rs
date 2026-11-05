#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::log::Lines;

use crate::cli::Locate;
use crate::cli::style::Palette;

use super::*;

pub(crate) fn test_app(apply: bool, locate: Locate) -> App<Lines<Vec<u8>, Vec<u8>>> {
    App {
        log: Lines::new(Vec::new(), Vec::new(), false),
        locate,
        palette: Palette::new(false),
        apply,
        recursive: false,
    }
}

pub(crate) fn output(app: App<Lines<Vec<u8>, Vec<u8>>>) -> String {
    let (out, _) = app.log.into_parts();
    String::from_utf8_lossy(&out).into_owned()
}

fn unused() -> Locate {
    Box::new(|_| anyhow::bail!("unused"))
}

fn item(dir: &str, files: &[&str], size: u64) -> Item {
    Item {
        dir: dir.to_owned(),
        files: files.iter().map(|f| (*f).to_owned()).collect(),
        size,
        warn: None,
        kept: None,
    }
}

fn kept(dir: &str, files: &[&str], size: u64, why: &str) -> Item {
    Item {
        kept: Some(why.to_owned()),
        ..item(dir, files, size)
    }
}

#[test]
fn show_prints_the_plan() {
    let one_dir = vec![
        item("", &["IMG_1264.RW2", "IMG_1264.RW2.pp3"], 24_012_000),
        item("", &["IMG_1271.RW2"], 25_000_000),
    ];
    let two_dirs = vec![
        item(
            "paris",
            &["paris/IMG_1264.RW2", "paris/IMG_1264.RW2.pp3"],
            24_012_000,
        ),
        item("lyon", &["lyon/IMG_2000.RW2"], 1_000_000),
        item("paris", &["paris/IMG_1271.RW2"], 25_000_000),
    ];
    let no_sizes = vec![
        item("", &["bordered/bordered_a.jpg"], 0),
        item("", &["bordered/bordered_b.jpg"], 0),
    ];
    let kept_items = vec![
        item(
            "paris",
            &["paris/IMG_1264.RW2", "paris/IMG_1264.RW2.pp3"],
            24_012_000,
        ),
        item("paris", &["paris/IMG_1271.RW2"], 25_000_000),
        kept(
            "paris",
            &["paris/IMG_1300.RW2"],
            2_000_000,
            "--exclude IMG_13*",
        ),
        kept(
            "marie/portrait",
            &["marie/portrait/IMG_1.RW2"],
            20_000_000,
            "--exclude *marie*",
        ),
        kept(
            "marie/portrait",
            &["marie/portrait/IMG_2.RW2"],
            20_000_000,
            "--exclude *marie*",
        ),
    ];
    let warned = vec![
        Item {
            warn: Some(Warn::EditedNeverExported),
            ..item("", &["IMG_1264.RW2", "IMG_1264.RW2.pp3"], 24_012_000)
        },
        item("", &["IMG_1271.RW2"], 25_000_000),
    ];

    let cases = [
        (
            "one directory",
            Verb::Delete,
            one_dir,
            "would delete IMG_1264.RW2\n\
            would delete IMG_1264.RW2.pp3\n\
            would delete IMG_1271.RW2\n\
            3 files, 49 MB. Re-run with --apply to delete them.\n",
        ),
        (
            "banner per directory",
            Verb::Delete,
            two_dirs,
            "####### paris: 3 files, 49 MB\n\
            would delete paris/IMG_1264.RW2\n\
            would delete paris/IMG_1264.RW2.pp3\n\
            would delete paris/IMG_1271.RW2\n\
            ####### lyon: 1 file, 1.0 MB\n\
            would delete lyon/IMG_2000.RW2\n\
            ####### recap\n\
            delete  paris  3 files, 49 MB\n\
            delete  lyon   1 file, 1.0 MB\n\
            4 files, 50 MB. Re-run with --apply to delete them.\n",
        ),
        (
            "kept items",
            Verb::Delete,
            kept_items,
            "####### paris: 3 files, 49 MB\n\
            would delete paris/IMG_1264.RW2\n\
            would delete paris/IMG_1264.RW2.pp3\n\
            would delete paris/IMG_1271.RW2\n\
            ####### recap\n\
            delete  paris           3 files, 49 MB\n\
            keep    paris           1 file, 2.0 MB  --exclude IMG_13*\n\
            keep    marie/portrait  2 files, 40 MB  --exclude *marie*\n\
            3 files, 49 MB, 3 files, 42 MB kept. Re-run with --apply to delete them.\n",
        ),
        (
            "no sizes",
            Verb::Write,
            no_sizes,
            "would write bordered/bordered_a.jpg\n\
            would write bordered/bordered_b.jpg\n\
            2 files. Re-run with --apply to write them.\n",
        ),
        (
            "warning banner",
            Verb::Delete,
            warned,
            "would delete IMG_1264.RW2  ####### has a .pp3 but nothing in converted/\n\
            would delete IMG_1264.RW2.pp3\n\
            would delete IMG_1271.RW2\n\
            ####### 1 file flagged: has a .pp3 but nothing in converted/\n\
            3 files, 49 MB. Re-run with --apply to delete them.\n",
        ),
    ];

    for (name, verb, items, want) in cases {
        let app = test_app(false, unused());
        assert!(
            !app.show(verb, &items, &[]),
            "{name}: dry run asked to apply"
        );
        assert_eq!(output(app), want, "{name}");
    }
}

#[test]
fn show_with_apply_prints_nothing_and_says_go() {
    let app = test_app(true, unused());
    let items = vec![
        item("", &["a"], 0),
        kept("", &["b"], 0, "b"),
        item("", &["c"], 0),
    ];
    assert!(app.show(Verb::Delete, &items, &[]));
    assert_eq!(output(app), "");
}

#[test]
fn show_skipped_and_nothing_to_run() {
    let app = test_app(false, unused());
    let items = vec![kept(
        "paris",
        &["paris/IMG_1264.RW2"],
        24_000_000,
        "not in --only *lyon*",
    )];
    let skips = vec![Skipped {
        dir: "liechtenstein".to_owned(),
        detail: "124 raws, 2.4 GB, no converted/: not sorted yet".to_owned(),
    }];
    app.show(Verb::Delete, &items, &skips);
    assert_eq!(
        output(app),
        "####### recap\n\
         keep    paris          1 file, 24 MB  not in --only *lyon*\n\
         skip    liechtenstein  124 raws, 2.4 GB, no converted/: not sorted yet\n\
         nothing to do, 1 file, 24 MB kept.\n"
    );
}

#[test]
fn recap_once_done() {
    let app = test_app(true, unused());
    let items = vec![
        item(".", &["IMG_1264.RW2", "IMG_1264.RW2.pp3"], 24_012_000),
        kept(".", &["IMG_1300.RW2"], 2_000_000, "--exclude IMG_13*"),
    ];
    assert_eq!(
        app.recap(Verb::Delete, &items, &[], Stage::Finished),
        [
            "####### recap",
            "deleted .  2 files, 24 MB",
            "keep    .  1 file, 2.0 MB  --exclude IMG_13*",
            "2 files deleted, 24 MB freed, 1 file, 2.0 MB kept.",
        ]
    );

    let one = vec![item(".", &["bordered/bordered_a.jpg"], 0)];
    assert_eq!(
        app.recap(Verb::Write, &one, &[], Stage::Finished),
        ["1 file written."]
    );
}

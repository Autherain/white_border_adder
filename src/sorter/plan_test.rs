#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::log::{MockLog, Silent};
use crate::testfs::{files, new_tree};
use jiff::civil::date;
use mockall::predicate::eq;

use super::super::{ExifOrMtime, MockDates};
use crate::sorter::exif::tests::{shot, tiff_with};

use super::*;

fn at(root: &VfsPath, p: &str) -> VfsPath {
    root.join(p).unwrap()
}

fn globs(texts: &[&str]) -> Vec<Glob> {
    texts.iter().map(|t| t.parse().unwrap()).collect()
}

fn lines(moves: &[Move]) -> Vec<String> {
    let mut out: Vec<String> = moves
        .iter()
        .map(|m| {
            let to = m.to.as_ref().map_or("", VfsPath::as_str);
            match &m.kept {
                Some(k) => format!("{} -> {to} [{k}]", m.from.as_str()),
                None => format!("{} -> {to}", m.from.as_str()),
            }
        })
        .collect();
    out.sort();
    out
}

fn plan_real(root: &VfsPath, sources: &[&str], o: &Options) -> Vec<Move> {
    let sources: Vec<VfsPath> = sources.iter().map(|s| at(root, s)).collect();
    plan(&Silent, &ExifOrMtime, &at(root, "r"), &sources, o).unwrap()
}

#[test]
fn the_always_excluded_globs_parse() {
    assert_eq!(globs(&ALWAYS_EXCLUDED).len(), 2);
}

#[test]
fn plan_files_photos_by_day() {
    struct Case {
        name: &'static str,
        tree: Vec<(&'static str, Vec<u8>)>,
        sources: &'static [&'static str],
        recursive: bool,
        want: &'static [&'static str],
    }

    let cases = [
        Case {
            name: "a dumped photo goes in the existing folder of its day, place name kept",
            tree: vec![
                ("r/dump/a.rw2", shot("0620")),
                ("r/2026/20260620_toulouse/b.rw2", shot("0620")),
            ],
            sources: &["r/dump"],
            recursive: false,
            want: &["/r/dump/a.rw2 -> /r/2026/20260620_toulouse/a.rw2"],
        },
        Case {
            name: "no folder for that day creates one named after the day",
            tree: vec![("r/dump/a.rw2", shot("0801"))],
            sources: &["r/dump"],
            recursive: false,
            want: &["/r/dump/a.rw2 -> /r/2026/20260801/a.rw2"],
        },
        Case {
            name: "a photo in a wrong day folder goes back to its day",
            tree: vec![
                ("r/2026/20260620_toulouse/a.rw2", shot("0701")),
                ("r/2026/20260701_lyon/b.rw2", shot("0701")),
            ],
            sources: &[],
            recursive: true,
            want: &["/r/2026/20260620_toulouse/a.rw2 -> /r/2026/20260701_lyon/a.rw2"],
        },
        Case {
            name: "a photo in the right folder is left alone, even below a subfolder",
            tree: vec![
                ("r/2026/20260620_toulouse/a.rw2", shot("0620")),
                ("r/2026/20260620_toulouse/sub/b.rw2", shot("0620")),
            ],
            sources: &[],
            recursive: true,
            want: &[],
        },
        Case {
            name: "the old layout without a year folder keeps its folder name",
            tree: vec![("r/20260620_toulouse/a.rw2", shot("0620"))],
            sources: &[],
            recursive: true,
            want: &["/r/20260620_toulouse/a.rw2 -> /r/2026/20260620_toulouse/a.rw2"],
        },
        Case {
            name: "the sidecar goes with its raw",
            tree: vec![
                ("r/d/a.rw2", shot("0801")),
                ("r/d/a.rw2.pp3", b"pp3".to_vec()),
            ],
            sources: &["r/d"],
            recursive: false,
            want: &["/r/d/a.rw2 -> /r/2026/20260801/a.rw2"],
        },
        Case {
            name: "converted and bordered are never read",
            tree: vec![
                ("r/d/converted/a.jpg", shot("0801")),
                ("r/d/bordered/b.jpg", shot("0801")),
                ("r/d/bordered_images/c.jpg", shot("0801")),
                ("r/d/.hidden/e.jpg", shot("0801")),
            ],
            sources: &[],
            recursive: true,
            want: &[],
        },
        Case {
            name: "never replaces a photo that already exists at the target",
            tree: vec![
                ("r/dump/a.rw2", shot("0620")),
                ("r/2026/20260620_toulouse/a.rw2", b"mine".to_vec()),
            ],
            sources: &["r/dump"],
            recursive: false,
            want: &[
                "/r/dump/a.rw2 -> /r/2026/20260620_toulouse/a.rw2 [already exists in 20260620_toulouse]",
            ],
        },
        Case {
            name: "two photos with the same name do not collide",
            tree: vec![
                ("r/dump/a/IMG_1.RW2", shot("0801")),
                ("r/dump/b/IMG_1.RW2", shot("0801")),
            ],
            sources: &["r/dump"],
            recursive: true,
            want: &[
                "/r/dump/a/IMG_1.RW2 -> /r/2026/20260801/IMG_1.RW2",
                "/r/dump/b/IMG_1.RW2 -> /r/2026/20260801/IMG_1.RW2 [same name as /r/dump/a/IMG_1.RW2]",
            ],
        },
        Case {
            name: "several folders for one day keep the photo and say so",
            tree: vec![
                ("r/dump/a.rw2", shot("0620")),
                ("r/2026/20260620_a/x.rw2", shot("0620")),
                ("r/2026/20260620_b/y.rw2", shot("0620")),
            ],
            sources: &["r/dump"],
            recursive: false,
            want: &["/r/dump/a.rw2 ->  [2 folders for 20260620 in 2026/: 20260620_a, 20260620_b]"],
        },
        Case {
            name: "not recursive stays in the directory",
            tree: vec![
                ("r/dump/a.rw2", shot("0801")),
                ("r/dump/sub/b.rw2", shot("0801")),
            ],
            sources: &["r/dump"],
            recursive: false,
            want: &["/r/dump/a.rw2 -> /r/2026/20260801/a.rw2"],
        },
    ];

    for case in cases {
        let tree: Vec<(&str, &[u8])> = case.tree.iter().map(|(p, c)| (*p, c.as_slice())).collect();
        let root = new_tree(&tree);
        let moves = plan_real(
            &root,
            case.sources,
            &Options {
                recursive: case.recursive,
                exclude: vec![],
            },
        );
        assert_eq!(lines(&moves), case.want, "{}", case.name);
    }
}

#[test]
fn converted_follows_its_original() {
    let root = new_tree(&[
        ("r/d/IMG_1.RW2", &shot("0801")),
        ("r/d/IMG_1.RW2.pp3", b"pp3"),
        ("r/d/converted/IMG_1.jpg", b"a"),
        ("r/d/converted/img_1-2.PNG", b"b"),
        ("r/d/converted/IMG_10.jpg", b"other"),
        ("r/d/bordered/bordered_IMG_1.jpg", b"never"),
    ]);

    let mut moves = plan_real(&root, &["r/d"], &Options::default());
    assert_eq!(moves.len(), 1, "IMG_10 has no original here");

    moves.remove(0).run().unwrap();
    assert_eq!(
        files(&root),
        [
            "r/2026/20260801/IMG_1.RW2",
            "r/2026/20260801/IMG_1.RW2.pp3",
            "r/2026/20260801/converted/IMG_1.jpg",
            "r/2026/20260801/converted/img_1-2.PNG",
            "r/d/bordered/bordered_IMG_1.jpg",
            "r/d/converted/IMG_10.jpg",
        ]
    );
}

#[test]
fn converted_never_overwrites() {
    let root = new_tree(&[
        ("r/d/IMG_1.RW2", &shot("0801")),
        ("r/d/converted/IMG_1.jpg", b"new"),
        ("r/2026/20260801/converted/IMG_1.jpg", b"mine"),
    ]);
    let moves = plan_real(&root, &["r/d"], &Options::default());
    assert_eq!(
        lines(&moves),
        ["/r/d/IMG_1.RW2 -> /r/2026/20260801/IMG_1.RW2 [already exists in 20260801]"]
    );
}

#[test]
fn run_from_inside_a_year_folder() {
    let root = new_tree(&[
        ("r/2026/20260620_toulouse/a.rw2", &shot("0620")),
        (
            "r/2026/20260620_toulouse/b.rw2",
            &tiff_with("2025:12:24 10:00:00"),
        ),
    ]);
    let moves = plan(
        &Silent,
        &ExifOrMtime,
        &at(&root, "r/2026"),
        &[],
        &Options {
            recursive: true,
            exclude: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        lines(&moves),
        ["/r/2026/20260620_toulouse/b.rw2 -> /r/2025/20251224/b.rw2"]
    );
}

#[test]
fn run_from_inside_a_year_folder_reads_that_folder_only() {
    let root = new_tree(&[
        (
            "r/2026/20260620_toulouse/a.rw2",
            &tiff_with("2025:12:24 10:00:00"),
        ),
        ("r/2025/dump/b.rw2", &shot("0801")),
    ]);
    let moves = plan(
        &Silent,
        &ExifOrMtime,
        &at(&root, "r/2026"),
        &[],
        &Options {
            recursive: true,
            exclude: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        lines(&moves),
        ["/r/2026/20260620_toulouse/a.rw2 -> /r/2025/20251224/a.rw2"]
    );
}

#[test]
fn exclude_leaves_photos_and_directories_alone_and_says_so() {
    let root = new_tree(&[
        ("r/dump/DSC_0001.NEF", &shot("0801")),
        ("r/dump/IMG_1.RW2", &shot("0801")),
        ("r/dump/Nikon old/b.jpg", &shot("0801")),
        ("r/dump/DSC_0001.NEF.pp3", b"pp3"),
    ]);

    let mut log = MockLog::new();
    log.expect_debug()
        .with(eq("skip dump/DSC_0001.NEF: --exclude dsc_*"))
        .times(1)
        .return_const(());

    let o = Options {
        recursive: true,
        exclude: globs(&["dsc_*", "*nikon*"]),
    };
    let moves = plan(
        &log,
        &ExifOrMtime,
        &at(&root, "r"),
        &[at(&root, "r/dump")],
        &o,
    )
    .unwrap();
    assert_eq!(
        lines(&moves),
        ["/r/dump/IMG_1.RW2 -> /r/2026/20260801/IMG_1.RW2"]
    );
}

#[test]
fn old_nikon_photos_never_move() {
    let root = new_tree(&[
        ("r/dump/DSC_0001.NEF", &shot("0801")),
        ("r/dump/Nikon D70/a.jpg", &shot("0801")),
    ]);
    let moves = plan_real(
        &root,
        &["r/dump"],
        &Options {
            recursive: true,
            exclude: vec![],
        },
    );
    assert!(moves.is_empty(), "{:?}", lines(&moves));
}

#[test]
fn plan_never_writes() {
    let root = new_tree(&[("r/dump/a.rw2", &shot("0801"))]);
    plan_real(&root, &["r/dump"], &Options::default());
    assert!(
        !at(&root, "r/2026").exists().unwrap_or(true),
        "planning created a directory"
    );
}

#[test]
fn a_photo_without_a_date_is_kept_and_listed() {
    let root = new_tree(&[("r/dump/a.png", b"png"), ("r/dump/b.png", b"png")]);

    let mut dates = MockDates::new();
    dates
        .expect_when()
        .withf(|p| p.filename() == "a.png")
        .return_const(None);
    dates
        .expect_when()
        .withf(|p| p.filename() == "b.png")
        .return_const(Some(date(2026, 6, 20).at(12, 0, 0, 0)));

    let moves = plan(
        &Silent,
        &dates,
        &at(&root, "r"),
        &[at(&root, "r/dump")],
        &Options::default(),
    )
    .unwrap();
    assert_eq!(
        lines(&moves),
        [
            "/r/dump/a.png ->  [no date]",
            "/r/dump/b.png -> /r/2026/20260620/b.png"
        ]
    );
}

#[test]
fn run_moves_raw_and_sidecar_and_never_overwrites() {
    let root = new_tree(&[
        ("r/dump/a.rw2", &shot("0801")),
        ("r/dump/a.rw2.pp3", b"pp3"),
    ]);

    let mut moves = plan_real(&root, &["r/dump"], &Options::default());
    moves.remove(0).run().unwrap();
    assert_eq!(
        files(&root),
        ["r/2026/20260801/a.rw2", "r/2026/20260801/a.rw2.pp3"]
    );

    let again = new_tree(&[
        ("r/dump/a.rw2", b"new"),
        ("r/2026/20260801/a.rw2", &shot("0801")),
    ]);
    let stale = Move {
        from: at(&again, "r/dump/a.rw2"),
        to: Some(at(&again, "r/2026/20260801/a.rw2")),
        with: vec![],
        kept: None,
    };
    let err = stale.run().expect_err("run replaced an existing file");
    assert!(
        matches!(err.source, crate::sorter::Error::Exists(_)),
        "{err}"
    );
    assert_eq!(
        crate::testfs::read(&again, "r/2026/20260801/a.rw2"),
        shot("0801"),
        "the existing file changed"
    );
}

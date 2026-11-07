#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::glob::Glob;
use crate::log::{MockLog, Silent};
use crate::testfs::{files, new_tree};
use mockall::Sequence;
use mockall::predicate::eq;

use super::*;

fn globs(texts: &[&str]) -> Vec<Glob> {
    texts.iter().map(|t| t.parse().unwrap()).collect()
}

fn options() -> Options {
    Options {
        pattern: globs(&["*.rw2"]).remove(0),
        exclude: vec![],
        only: vec![],
        recursive: false,
        bordered: false,
    }
}

fn at(root: &VfsPath, p: &str) -> VfsPath {
    root.join(p).unwrap()
}

fn to_delete(plan: Plan) -> Vec<Candidate> {
    plan.candidates
        .into_iter()
        .filter(|c| c.kept.is_none())
        .collect()
}

fn raw_paths(cs: &[Candidate]) -> Vec<String> {
    cs.iter()
        .map(|c| c.raw.path.as_str().trim_start_matches('/').to_owned())
        .collect()
}

struct Case {
    name: &'static str,
    tree: Vec<(&'static str, &'static [u8])>,
    dir: &'static str,
    opts: Options,
    want: Vec<&'static str>,
    /// Every file delete removes, raws and sidecars.
    files: Vec<&'static str>,
}

#[test]
fn plan_then_delete() {
    let cases = [
        Case {
            name: "developed raws are kept whatever the converted name",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/IMG_1265.RW2", b"raw"),
                ("paris/IMG_1266.RW2", b"raw"),
                ("paris/IMG_1267.RW2", b"raw"),
                ("paris/converted/IMG_1264.jpg", b"jpeg"),
                ("paris/converted/IMG_1265_converted.jpg", b"jpeg"),
                ("paris/converted/img_1266-bw.tif", b"tiff"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/IMG_1267.RW2"],
            files: vec!["paris/IMG_1267.RW2"],
        },
        Case {
            name: "stem must be a prefix, extension alone does not count",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/converted/IMG_9999.RW2", b"raw"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/IMG_1264.RW2"],
            files: vec!["paris/IMG_1264.RW2"],
        },
        Case {
            name: "sidecar goes with its raw and stays with a kept raw",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/IMG_1264.RW2.pp3", b"sidecar"),
                ("paris/IMG_1271.RW2", b"raw"),
                ("paris/IMG_1271.RW2.pp3", b"sidecar"),
                ("paris/converted/IMG_1264.jpg", b"jpeg"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/IMG_1271.RW2"],
            files: vec!["paris/IMG_1271.RW2", "paris/IMG_1271.RW2.pp3"],
        },
        Case {
            name: "excluded stems leave the list",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/IMG_1271.RW2", b"raw"),
                ("paris/IMG_1300.RW2", b"raw"),
                ("paris/IMG_2000.RW2", b"raw"),
                ("paris/converted/IMG_0001.jpg", b"jpeg"),
            ],
            dir: "paris",
            opts: Options {
                exclude: globs(&["IMG_12*", "IMG_1300"]),
                ..options()
            },
            want: vec!["paris/IMG_2000.RW2"],
            files: vec!["paris/IMG_2000.RW2"],
        },
        Case {
            name: "no converted directory means nothing is a candidate",
            tree: vec![
                ("lyon/IMG_2000.RW2", b"raw"),
                ("lyon/converted", b"a file, not a directory"),
            ],
            dir: "lyon",
            opts: options(),
            want: vec![],
            files: vec![],
        },
        Case {
            name: "only the root of the photo directory is scanned",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/converted/IMG_5000.RW2", b"raw"),
                ("paris/backup/IMG_6000.RW2", b"raw"),
                ("paris/converted/IMG_0001.jpg", b"jpeg"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/IMG_1264.RW2"],
            files: vec!["paris/IMG_1264.RW2"],
        },
        Case {
            name: "pattern is case-insensitive and other files are ignored",
            tree: vec![
                ("paris/a.rw2", b"raw"),
                ("paris/B.RW2", b"raw"),
                ("paris/c.jpg", b"jpeg"),
                ("paris/converted/IMG_0001.jpg", b"jpeg"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/B.RW2", "paris/a.rw2"],
            files: vec!["paris/B.RW2", "paris/a.rw2"],
        },
        Case {
            name: "recursive visits every photo directory below the argument",
            tree: vec![
                ("2024-10-paris/IMG_1264.RW2", b"raw"),
                ("2024-10-paris/IMG_1264.RW2.pp3", b"sidecar"),
                ("2024-10-paris/IMG_1271.RW2", b"raw"),
                ("2024-10-paris/converted/IMG_1264.jpg", b"jpeg"),
                ("2024-11-lyon/IMG_2000.RW2", b"raw"),
                ("2024-11-lyon/day2/IMG_2100.RW2", b"raw"),
                ("2024-11-lyon/day2/converted/x.jpg", b"jpeg"),
                ("2024-11-lyon/day2/converted/IMG.RW2", b"raw"),
                (".cache/IMG_3000.RW2", b"raw"),
                (".cache/converted/x.jpg", b"jpeg"),
                ("IMG_0001.RW2", b"raw"),
            ],
            dir: "",
            opts: Options {
                recursive: true,
                ..options()
            },
            want: vec![
                "2024-10-paris/IMG_1271.RW2",
                "2024-11-lyon/day2/IMG_2100.RW2",
            ],
            files: vec![
                "2024-10-paris/IMG_1271.RW2",
                "2024-11-lyon/day2/IMG_2100.RW2",
            ],
        },
        Case {
            name: "bordered copies stay without --bordered",
            tree: vec![
                ("paris/converted/a.jpg", b"jpeg"),
                ("paris/bordered/bordered_a.jpg", b"ig"),
            ],
            dir: "paris",
            opts: Options {
                recursive: true,
                ..options()
            },
            want: vec![],
            files: vec![],
        },
        Case {
            name: "--bordered deletes only the Instagram copies",
            tree: vec![
                ("shoot/converted/a.jpg", b"original"),
                ("shoot/converted/bordered/bordered_a.jpg", b"ig"),
                ("shoot/bordered/bordered_b.png", b"ig"),
                ("shoot/bordered/notes.txt", b"mine"),
                ("shoot/bordered_images/c.JPG", b"ig"),
                ("shoot/IMG_1.RW2", b"raw"),
                ("shoot/converted/IMG_1.jpg", b"developed"),
                ("nosort/bordered/bordered_d.jpg", b"ig"),
                ("shoot/.hidden/bordered/bordered_e.jpg", b"ig"),
            ],
            dir: "",
            opts: Options {
                recursive: true,
                bordered: true,
                ..options()
            },
            want: vec![
                "nosort/bordered/bordered_d.jpg",
                "shoot/bordered/bordered_b.png",
                "shoot/bordered_images/c.JPG",
                "shoot/converted/bordered/bordered_a.jpg",
            ],
            files: vec![
                "nosort/bordered/bordered_d.jpg",
                "shoot/bordered/bordered_b.png",
                "shoot/bordered_images/c.JPG",
                "shoot/converted/bordered/bordered_a.jpg",
            ],
        },
        Case {
            name: "--bordered follows --exclude",
            tree: vec![
                ("20260620_marie/converted/x.jpg", b"x"),
                ("20260620_marie/converted/bordered/bordered_x.jpg", b"ig"),
                ("20260701_lyon/converted/y.jpg", b"y"),
                ("20260701_lyon/converted/bordered/bordered_y.jpg", b"ig"),
            ],
            dir: "",
            opts: Options {
                recursive: true,
                bordered: true,
                exclude: globs(&["*marie*"]),
                ..options()
            },
            want: vec!["20260701_lyon/converted/bordered/bordered_y.jpg"],
            files: vec!["20260701_lyon/converted/bordered/bordered_y.jpg"],
        },
        Case {
            name: "non-recursive stops at the argument",
            tree: vec![
                ("paris/IMG_1264.RW2", b"raw"),
                ("paris/converted/x.jpg", b"jpeg"),
                ("paris/day2/IMG_2100.RW2", b"raw"),
                ("paris/day2/converted/x.jpg", b"jpeg"),
                ("paris/converted/IMG_0001.jpg", b"jpeg"),
            ],
            dir: "paris",
            opts: options(),
            want: vec!["paris/IMG_1264.RW2"],
            files: vec!["paris/IMG_1264.RW2"],
        },
    ];

    for case in cases {
        let root = new_tree(&case.tree);
        let before = files(&root);

        let plan = plan(&Silent, &at(&root, case.dir), &case.opts)
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let got = to_delete(plan);
        assert_eq!(raw_paths(&got), case.want, "{}", case.name);
        assert_eq!(files(&root), before, "{}: plan changed the tree", case.name);

        let mut removed: Vec<String> = got
            .into_iter()
            .flat_map(|c| c.delete().unwrap_or_else(|e| panic!("{}: {e}", case.name)))
            .map(|p| p.as_str().trim_start_matches('/').to_owned())
            .collect();
        removed.sort();
        assert_eq!(removed, case.files, "{}", case.name);

        let want: Vec<&String> = before.iter().filter(|p| !removed.contains(p)).collect();
        assert_eq!(
            files(&root).iter().collect::<Vec<_>>(),
            want,
            "{}: tree after delete",
            case.name
        );
    }
}

#[test]
fn sizes() {
    let root = new_tree(&[
        ("paris/IMG_1271.RW2", b"12345"),
        ("paris/IMG_1271.RW2.pp3", b"abc"),
        ("paris/converted/x.jpg", b"jpeg"),
    ]);
    let plan = plan(&Silent, &at(&root, "paris"), &options()).unwrap();
    let want = vec![Candidate {
        raw: File {
            path: at(&root, "paris/IMG_1271.RW2"),
            size: 5,
        },
        sidecar: Some(File {
            path: at(&root, "paris/IMG_1271.RW2.pp3"),
            size: 3,
        }),
        kept: None,
        bordered: false,
    }];
    assert_eq!(plan.candidates, want);
    assert_eq!(plan.candidates[0].size(), 8);
}

#[test]
fn deleting_bordered_copies_removes_the_empty_folder_only() {
    let root = new_tree(&[
        ("a/converted/bordered/bordered_1.jpg", b"ig"),
        ("b/bordered/bordered_2.jpg", b"ig"),
        ("b/bordered/notes.txt", b"mine"),
        ("a/converted/1.jpg", b"original"),
    ]);
    let plan = plan(
        &Silent,
        &root,
        &Options {
            recursive: true,
            bordered: true,
            ..options()
        },
    )
    .unwrap();

    for c in to_delete(plan) {
        c.delete().unwrap();
    }

    assert!(
        !at(&root, "a/converted/bordered").exists().unwrap(),
        "the emptied bordered folder stayed"
    );
    assert_eq!(
        files(&root),
        ["a/converted/1.jpg", "b/bordered/notes.txt"],
        "only the copies went"
    );
}

#[test]
fn a_missing_directory_or_a_file_is_an_error() {
    let root = new_tree(&[("paris/converted/x.jpg", b"jpeg"), ("file.txt", b"x")]);
    for dir in ["nowhere", "file.txt"] {
        assert!(plan(&Silent, &at(&root, dir), &options()).is_err(), "{dir}");
    }
}

#[test]
fn verbose_names_the_directories_without_converted() {
    let root = new_tree(&[
        ("2024-10-paris/IMG_1264.RW2", b"raw"),
        ("2024-10-paris/converted/x.jpg", b"jpeg"),
        ("2024-11-lyon/IMG_2000.RW2", b"raw"),
    ]);

    let mut log = MockLog::new();
    let mut order = Sequence::new();
    for line in ["skip .: no converted/", "skip 2024-11-lyon: no converted/"] {
        log.expect_debug()
            .with(eq(line))
            .times(1)
            .in_sequence(&mut order)
            .return_const(());
    }

    plan(
        &log,
        &root,
        &Options {
            recursive: true,
            ..options()
        },
    )
    .unwrap();
}

/// The owner's real tree: one directory per shoot, raws and sidecars at
/// its root, exports in converted/, and the outputs of wba and of the
/// script before it under converted/.
fn raw_therapee_layout() -> VfsPath {
    new_tree(&[
        ("2025/20250804_luchon/_1310721.RW2", b"raw"),
        ("2025/20250804_luchon/_1310721.RW2.pp3", b"sidecar"),
        ("2025/20250804_luchon/_1310726.RW2", b"raw"),
        ("2025/20250804_luchon/_1310752.RW2", b"raw"),
        (
            "2025/20250804_luchon/converted/_1310721_converted.jpg",
            b"jpeg",
        ),
        (
            "2025/20250804_luchon/converted/bordered_images/bordered__1310721.jpg",
            b"jpeg",
        ),
        (
            "2025/20250804_luchon/converted/bordered/bordered__1310721.jpg",
            b"jpeg",
        ),
        ("2025/20250715_toulouse/_1300001.RW2", b"raw"),
        ("2025/20251129_pot_depart1_gaspard/_1330976.JPG", b"jpeg"),
        ("2025/.stfolder/marker", b""),
    ])
}

#[test]
fn raw_therapee_layout_is_cleaned_shoot_by_shoot() {
    let root = raw_therapee_layout();
    let plan = plan(
        &Silent,
        &at(&root, "2025"),
        &Options {
            recursive: true,
            ..options()
        },
    )
    .unwrap();

    // The shoot without converted/ is reported, with what waits in it.
    assert_eq!(
        plan.skipped,
        vec![Skipped {
            dir: at(&root, "2025/20250715_toulouse"),
            raws: 1,
            size: 3
        }]
    );

    // _1310721 is developed, 20250715_toulouse has no converted/, the JPEG
    // shoot has no raws: only the two undeveloped Luchon raws remain.
    assert_eq!(
        raw_paths(&to_delete(plan)),
        [
            "2025/20250804_luchon/_1310726.RW2",
            "2025/20250804_luchon/_1310752.RW2"
        ]
    );
}

#[test]
fn raw_therapee_layout_bordered_adds_the_copies_of_both_scripts() {
    let root = raw_therapee_layout();
    let plan = plan(
        &Silent,
        &at(&root, "2025"),
        &Options {
            recursive: true,
            bordered: true,
            ..options()
        },
    )
    .unwrap();

    assert_eq!(
        raw_paths(&to_delete(plan)),
        [
            "2025/20250804_luchon/_1310726.RW2",
            "2025/20250804_luchon/_1310752.RW2",
            "2025/20250804_luchon/converted/bordered/bordered__1310721.jpg",
            "2025/20250804_luchon/converted/bordered_images/bordered__1310721.jpg",
        ]
    );
}

#[test]
fn exclude_keeps_by_directory_stem_or_path() {
    let cases: [(&str, &[&str], &[&str]); 4] = [
        ("by shoot directory", &["*luchon*"], &[]),
        (
            "by stem",
            &["_1310726"],
            &["2025/20250804_luchon/_1310752.RW2"],
        ),
        (
            "by path",
            &["20250804_luchon/_131075*"],
            &["2025/20250804_luchon/_1310726.RW2"],
        ),
        (
            "no match",
            &["luchon"],
            &[
                "2025/20250804_luchon/_1310726.RW2",
                "2025/20250804_luchon/_1310752.RW2",
            ],
        ),
    ];

    for (name, exclude, want) in cases {
        let root = raw_therapee_layout();
        let opts = Options {
            recursive: true,
            exclude: globs(exclude),
            ..options()
        };
        let plan = plan(&Silent, &at(&root, "2025"), &opts).unwrap();

        // Kept candidates stay in the plan, marked with the glob that kept them.
        for c in &plan.candidates {
            if let Some(kept) = &c.kept {
                let g = match kept {
                    Kept::Excluded(g) => g,
                    Kept::NotInOnly(_) => panic!("{name}: kept by {kept}"),
                };
                assert!(exclude.contains(&g.as_str()), "{name}: kept by {g}");
            }
        }
        assert_eq!(raw_paths(&to_delete(plan)), want, "{name}");
    }
}

#[test]
fn only_keeps_everything_else_and_exclude_wins() {
    struct OnlyCase {
        name: &'static str,
        only: &'static [&'static str],
        exclude: &'static [&'static str],
        want: &'static [&'static str],
        kept: &'static str,
    }

    let cases = [
        OnlyCase {
            name: "by shoot directory",
            only: &["*luchon*"],
            exclude: &[],
            want: &[
                "2025/20250804_luchon/_1310726.RW2",
                "2025/20250804_luchon/_1310752.RW2",
            ],
            kept: "",
        },
        OnlyCase {
            name: "by stem",
            only: &["_1310726"],
            exclude: &[],
            want: &["2025/20250804_luchon/_1310726.RW2"],
            kept: "not in --only _1310726",
        },
        OnlyCase {
            name: "no match keeps everything",
            only: &["*paris*"],
            exclude: &[],
            want: &[],
            kept: "not in --only *paris*",
        },
        OnlyCase {
            name: "exclude wins over only",
            only: &["*luchon*"],
            exclude: &["_1310726"],
            want: &["2025/20250804_luchon/_1310752.RW2"],
            kept: "--exclude _1310726",
        },
    ];

    for case in cases {
        let root = raw_therapee_layout();
        let opts = Options {
            recursive: true,
            only: globs(case.only),
            exclude: globs(case.exclude),
            ..options()
        };
        let plan = plan(&Silent, &at(&root, "2025"), &opts).unwrap();

        for c in &plan.candidates {
            if let Some(k) = &c.kept {
                assert_eq!(
                    k.to_string(),
                    case.kept,
                    "{}: {}",
                    case.name,
                    c.raw.path.as_str()
                );
            }
        }
        assert_eq!(raw_paths(&to_delete(plan)), case.want, "{}", case.name);
    }
}

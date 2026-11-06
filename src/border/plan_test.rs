#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::log::{MockLog, Silent};
use crate::testfs::new_tree;
use mockall::predicate::eq;

use super::super::Ratios;

use super::*;

fn options() -> Options {
    Options {
        width: 1080,
        height: 1080,
        landscape: Ratios {
            vert: 0.05,
            horiz: 0.03,
        },
        portrait: Ratios {
            vert: 0.005,
            horiz: 0.18,
        },
        jpeg_quality: 100,
        prefix: "bordered_".to_owned(),
        out: Out::default(),
        recursive: false,
        force: false,
    }
}

fn tree() -> VfsPath {
    new_tree(&[
        ("photos/IMG_1.jpg", b"jpeg"),
        ("photos/IMG_2.JPEG", b"jpeg"),
        ("photos/IMG_3.png", b"png"),
        ("photos/IMG_4.RW2", b"raw"),
        ("photos/notes.txt", b"text"),
        ("photos/converted/IMG_5.jpg", b"jpeg"),
        ("photos/bordered/bordered_IMG_1.jpg", b"jpeg"),
        ("photos/.cache/IMG_6.jpg", b"jpeg"),
        ("photos/2024-10-paris/IMG_7.jpg", b"jpeg"),
    ])
}

fn pairs(jobs: &[Job]) -> Vec<(&str, &str)> {
    jobs.iter()
        .map(|j| (j.input.as_str(), j.output.as_str()))
        .collect()
}

#[test]
fn plan_lists_images_and_where_they_go() {
    struct Case {
        name: &'static str,
        opts: Options,
        want: Vec<(&'static str, &'static str)>,
        skip: Option<&'static str>,
    }

    let cases = [
        Case {
            name: "non-recursive skips existing output",
            opts: options(),
            want: vec![
                ("/photos/IMG_2.JPEG", "/photos/bordered/bordered_IMG_2.JPEG"),
                ("/photos/IMG_3.png", "/photos/bordered/bordered_IMG_3.png"),
            ],
            skip: Some("skip IMG_1.jpg: output exists"),
        },
        Case {
            name: "force overwrites existing output",
            opts: Options {
                force: true,
                ..options()
            },
            want: vec![
                ("/photos/IMG_1.jpg", "/photos/bordered/bordered_IMG_1.jpg"),
                ("/photos/IMG_2.JPEG", "/photos/bordered/bordered_IMG_2.JPEG"),
                ("/photos/IMG_3.png", "/photos/bordered/bordered_IMG_3.png"),
            ],
            skip: None,
        },
        Case {
            name: "recursive enters converted, skips bordered and dot dirs",
            opts: Options {
                recursive: true,
                ..options()
            },
            want: vec![
                (
                    "/photos/2024-10-paris/IMG_7.jpg",
                    "/photos/2024-10-paris/bordered/bordered_IMG_7.jpg",
                ),
                ("/photos/IMG_2.JPEG", "/photos/bordered/bordered_IMG_2.JPEG"),
                ("/photos/IMG_3.png", "/photos/bordered/bordered_IMG_3.png"),
                (
                    "/photos/converted/IMG_5.jpg",
                    "/photos/converted/bordered/bordered_IMG_5.jpg",
                ),
            ],
            skip: Some("skip IMG_1.jpg: output exists"),
        },
        Case {
            name: "custom relative out dir",
            opts: Options {
                out: Out::Beside("insta".to_owned()),
                ..options()
            },
            want: vec![
                ("/photos/IMG_1.jpg", "/photos/insta/bordered_IMG_1.jpg"),
                ("/photos/IMG_2.JPEG", "/photos/insta/bordered_IMG_2.JPEG"),
                ("/photos/IMG_3.png", "/photos/insta/bordered_IMG_3.png"),
            ],
            skip: None,
        },
    ];

    for case in cases {
        let root = tree();
        let mut log = MockLog::new();
        match case.skip {
            Some(line) => log.expect_debug().with(eq(line)).times(1).return_const(()),
            None => log.expect_debug().never(),
        };

        let dir = root.join("photos").unwrap();
        let jobs = plan(&log, &dir, &case.opts).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        assert_eq!(pairs(&jobs), case.want, "{}", case.name);
    }
}

#[test]
fn one_out_dir_for_every_photo() {
    let root = tree();
    let out = root.join("all").unwrap();
    let dir = root.join("photos").unwrap();

    let jobs = plan(
        &Silent,
        &dir,
        &Options {
            recursive: true,
            out: Out::At(out),
            ..options()
        },
    )
    .unwrap();
    let outputs: Vec<&str> = jobs.iter().map(|j| j.output.as_str()).collect();
    // Only the current output directory is skipped: an earlier run's
    // bordered/ is a photo directory like any other.
    assert_eq!(
        outputs,
        [
            "/all/bordered_IMG_7.jpg",
            "/all/bordered_IMG_1.jpg",
            "/all/bordered_IMG_2.JPEG",
            "/all/bordered_IMG_3.png",
            "/all/bordered_bordered_IMG_1.jpg",
            "/all/bordered_IMG_5.jpg"
        ]
    );
}

#[test]
fn a_missing_directory_or_a_file_is_an_error() {
    let root = new_tree(&[("photos/IMG_1.jpg", b"jpeg")]);
    for dir in ["nowhere", "photos/IMG_1.jpg"] {
        let dir = root.join(dir).unwrap();
        assert!(plan(&Silent, &dir, &options()).is_err(), "{}", dir.as_str());
    }
}

/// The owner's real tree: one directory per shoot, raws and sidecars at
/// its root, exports in converted/, and the outputs of wba and of the
/// script before it under converted/.
#[test]
fn raw_therapee_layout_borders_the_exports_only() {
    let root = new_tree(&[
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
    ]);
    let dir = root.join("2025").unwrap();

    let jobs = plan(
        &Silent,
        &dir,
        &Options {
            recursive: true,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(
        pairs(&jobs),
        [
            (
                "/2025/20250804_luchon/converted/_1310721_converted.jpg",
                "/2025/20250804_luchon/converted/bordered/bordered__1310721_converted.jpg"
            ),
            (
                "/2025/20251129_pot_depart1_gaspard/_1330976.JPG",
                "/2025/20251129_pot_depart1_gaspard/bordered/bordered__1330976.JPG"
            ),
        ]
    );
}

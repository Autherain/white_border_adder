#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::testfs::{files, new_tree};

use crate::cli::show::tests::{output, test_app};

use super::*;

fn args(dir: &str, exclude: &[&str]) -> CleanArgs {
    CleanArgs {
        dir: dir.to_owned(),
        pattern: "*.rw2".parse().unwrap(),
        exclude: exclude.iter().map(|g| g.parse().unwrap()).collect(),
        only: vec![],
        bordered: false,
    }
}

#[test]
fn clean_dry_run_then_apply() {
    for apply in [false, true] {
        let root = new_tree(&[
            ("paris/IMG_1264.RW2", b"raw"),
            ("paris/IMG_1271.RW2", b"raw"),
            ("paris/IMG_1271.RW2.pp3", b"pp3"),
            ("paris/IMG_1300.RW2", b"raw"),
            ("paris/converted/IMG_1264.jpg", b"jpeg"),
        ]);
        let at = root.clone();
        let (app, out) = test_app(apply, Box::new(move |p| Ok(at.join(p)?)));

        app.clean(args("paris", &["IMG_13*"])).unwrap();

        let want = if apply {
            "deleted IMG_1271.RW2\n\
             deleted IMG_1271.RW2.pp3\n\
             ####### recap\n\
             deleted .  2 files, 6 B\n\
             keep    .  1 file, 3 B  --exclude IMG_13*\n\
             ####### 1 file flagged: has a .pp3 but nothing in converted/\n\
             2 files deleted, 6 B freed, 1 file, 3 B kept.\n"
        } else {
            "would delete IMG_1271.RW2  ####### has a .pp3 but nothing in converted/\n\
             would delete IMG_1271.RW2.pp3\n\
             ####### recap\n\
             delete  .  2 files, 6 B\n\
             keep    .  1 file, 3 B  --exclude IMG_13*\n\
             ####### 1 file flagged: has a .pp3 but nothing in converted/\n\
             2 files, 6 B, 1 file, 3 B kept. Re-run with --apply to delete them.\n"
        };
        assert_eq!(output(&out), want, "apply={apply}");
        assert_eq!(
            files(&root).contains(&"paris/IMG_1271.RW2".to_owned()),
            !apply,
            "apply={apply}"
        );
    }
}

#[test]
fn bordered_copies_are_listed_under_their_folder() {
    let root = new_tree(&[
        ("paris/converted/IMG_1264.jpg", b"jpeg"),
        ("paris/converted/bordered/bordered_IMG_1264.jpg", b"ig"),
        ("paris/bordered/bordered_IMG_1265.jpg", b"ig"),
    ]);
    let (app, out) = test_app(false, Box::new(move |p| Ok(root.join(p)?)));

    app.clean(CleanArgs {
        bordered: true,
        ..args("paris", &[])
    })
    .unwrap();

    assert_eq!(
        output(&out),
        "####### bordered: 1 file, 2 B\n\
         would delete bordered/bordered_IMG_1265.jpg\n\
         ####### converted/bordered: 1 file, 2 B\n\
         would delete converted/bordered/bordered_IMG_1264.jpg\n\
         ####### recap\n\
         delete  bordered            1 file, 2 B\n\
         delete  converted/bordered  1 file, 2 B\n\
         2 files, 4 B. Re-run with --apply to delete them.\n"
    );
}

#[test]
fn nothing_to_do() {
    let root = new_tree(&[("paris/converted/x.jpg", b"jpeg")]);
    let (app, out) = test_app(false, Box::new(move |p| Ok(root.join(p)?)));
    app.clean(args("paris", &[])).unwrap();
    assert_eq!(output(&out), "nothing to do\n");
}

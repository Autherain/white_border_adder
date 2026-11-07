#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::testfs::{files, new_tree};

use crate::cli::show::tests::{output, test_app};

use super::*;

fn args(dir: &str) -> BorderArgs {
    BorderArgs {
        dir: dir.to_owned(),
        width: 64,
        height: 64,
        landscape_vert: 0.05,
        landscape_horiz: 0.03,
        portrait_vert: 0.005,
        portrait_horiz: 0.18,
        jpeg_quality: 90,
        prefix: "bordered_".to_owned(),
        out: None,
        force: false,
        workers: 2,
    }
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 4, image::Rgb([0, 0, 0]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

#[test]
fn border_dry_run_then_apply() {
    for apply in [false, true] {
        let root = new_tree(&[("photos/a.png", &png()), ("photos/b.jpg", b"not a jpeg")]);
        let at = root.clone();
        let (app, out) = test_app(apply, Box::new(move |p| Ok(at.join(p)?)));

        let result = app.border(args("photos"));
        let got = output(&out);

        if apply {
            assert_eq!(
                result.err().map(|e| e.to_string()),
                Some("1 file failed".to_owned())
            );
            assert_eq!(got, "wrote bordered/bordered_a.png\n2 files written.\n");
            assert!(files(&root).contains(&"photos/bordered/bordered_a.png".to_owned()));
        } else {
            result.unwrap();
            assert_eq!(
                got,
                "would write bordered/bordered_a.png\n\
                 would write bordered/bordered_b.jpg\n\
                 2 files. Re-run with --apply to write them.\n"
            );
        }
    }
}

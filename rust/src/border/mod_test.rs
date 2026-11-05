#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::testfs::{new_tree, read};
use image::{ImageBuffer, Rgb};

use super::*;
use render::layout;

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

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = ImageBuffer::from_pixel(w, h, Rgb([200u8, 10, 10]));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
    buf.into_inner()
}

#[test]
fn run_writes_the_photo_centred_on_a_white_canvas() {
    let root = new_tree(&[("photos/IMG_1.jpg", &jpeg(40, 30))]);
    let job = Job {
        input: root.join("photos/IMG_1.jpg").unwrap(),
        output: root.join("photos/bordered/bordered_IMG_1.jpg").unwrap(),
    };

    let out = job.run(&options()).unwrap();
    assert_eq!(out.as_str(), "/photos/bordered/bordered_IMG_1.jpg");

    let img = image::load_from_memory(&read(&root, "photos/bordered/bordered_IMG_1.jpg"))
        .unwrap()
        .to_rgb8();
    assert_eq!(img.dimensions(), (1080, 1080));
    assert_eq!(
        img.get_pixel(5, 5).0,
        [255, 255, 255],
        "the border is white"
    );
    let [r, g, b] = img.get_pixel(540, 540).0;
    assert!(
        r > 150 && g < 60 && b < 60,
        "the photo sits in the middle: {r} {g} {b}"
    );
}

#[test]
fn run_reports_a_file_it_cannot_decode() {
    let root = new_tree(&[("photos/IMG_1.jpg", b"not a jpeg")]);
    let job = Job {
        input: root.join("photos/IMG_1.jpg").unwrap(),
        output: root.join("photos/bordered/bordered_IMG_1.jpg").unwrap(),
    };

    let err = job.run(&options()).expect_err("expected an error");
    assert_eq!(err.input.as_str(), "/photos/IMG_1.jpg");
    assert!(matches!(err.source, Error::DecodeJpeg(_)), "{err}");
}

#[test]
fn a_png_keeps_its_transparency_over_the_white() {
    // A 20x10 PNG, fully transparent: the output must be all white.
    let img = image::RgbaImage::from_pixel(20, 10, image::Rgba([0, 0, 0, 0]));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    let root = new_tree(&[("photos/IMG_1.png", buf.get_ref())]);
    let job = Job {
        input: root.join("photos/IMG_1.png").unwrap(),
        output: root.join("photos/bordered/bordered_IMG_1.png").unwrap(),
    };

    job.run(&options()).unwrap();

    let out = image::load_from_memory(&read(&root, "photos/bordered/bordered_IMG_1.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(out.dimensions(), (1080, 1080));
    assert_eq!(out.get_pixel(540, 540).0, [255, 255, 255, 255]);
}

#[test]
fn a_big_jpeg_is_decoded_reduced_and_still_fills_the_frame() {
    // 4000x3000 lands on a 1015x761 frame; the decoder hands back 1/2 or 1/4
    // of the picture and the frame must not change.
    let img = image::RgbImage::from_pixel(4000, 3000, image::Rgb([10, 10, 200]));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
    let root = new_tree(&[("photos/big.jpg", buf.get_ref())]);
    let job = Job {
        input: root.join("photos/big.jpg").unwrap(),
        output: root.join("out/big.jpg").unwrap(),
    };

    job.run(&options()).unwrap();

    let out = image::load_from_memory(&read(&root, "out/big.jpg"))
        .unwrap()
        .to_rgb8();
    let at = layout(4000, 3000, &options());
    let inside = out.get_pixel(at.x + 2, at.y + 2).0;
    let outside = out.get_pixel(at.x - 2, at.y - 2).0;
    assert!(
        inside[2] > 150 && inside[0] < 60,
        "inside the frame is blue: {inside:?}"
    );
    assert_eq!(outside, [255, 255, 255], "outside the frame is white");
}

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::testfs::new_tree;
use jiff::civil::date;

use super::*;

/// The head of a TIFF-like file whose EXIF says it was shot at `stamp`
/// ("2026:06:20 10:00:00"), the way a camera raw lays it out.
pub fn tiff_with(stamp: &str) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..2].copy_from_slice(b"II");
    b[2..4].copy_from_slice(&42u16.to_le_bytes());
    b[4..8].copy_from_slice(&8u32.to_le_bytes());
    b[8..10].copy_from_slice(&1u16.to_le_bytes());
    entry(&mut b[10..22], TAG_EXIF_IFD, TYPE_LONG, 1, 30);
    b[30..32].copy_from_slice(&1u16.to_le_bytes());
    entry(&mut b[32..44], TAG_DATE_TIME_ORIGINAL, TYPE_ASCII, 20, 60);
    b[60..60 + stamp.len()].copy_from_slice(stamp.as_bytes());
    b
}

fn entry(e: &mut [u8], tag: u16, kind: u16, count: u32, value: u32) {
    e[..2].copy_from_slice(&tag.to_le_bytes());
    e[2..4].copy_from_slice(&kind.to_le_bytes());
    e[4..8].copy_from_slice(&count.to_le_bytes());
    e[8..12].copy_from_slice(&value.to_le_bytes());
}

/// Shot in 2026 on `day` ("0620").
pub fn shot(day: &str) -> Vec<u8> {
    tiff_with(&format!("2026:{}:{} 10:00:00", &day[..2], &day[2..]))
}

#[test]
fn reads_the_date_the_camera_wrote() {
    let root = new_tree(&[("a.rw2", &shot("0620")), ("b.png", b"no exif")]);
    let at = |p: &str| root.join(p).unwrap();

    assert_eq!(
        shot_date(&at("a.rw2")),
        Some(date(2026, 6, 20).at(10, 0, 0, 0))
    );
    assert_eq!(
        shot_date(&at("b.png")),
        None,
        "a file without EXIF has no shot date"
    );
}

#[test]
fn a_jpeg_carries_it_in_app1() {
    let tiff = shot("0801");
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    let size = u16::try_from(2 + 6 + tiff.len()).unwrap();
    jpeg.extend_from_slice(&size.to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);

    let root = new_tree(&[("a.jpg", &jpeg)]);
    let path = root.join("a.jpg").unwrap();
    assert_eq!(shot_date(&path), Some(date(2026, 8, 1).at(10, 0, 0, 0)));
}

#[test]
fn garbage_never_panics() {
    for bytes in [
        &b"II"[..],
        b"II\x2a\x00\xff\xff\xff\xff",
        b"MM\x00\x2a\x00\x00\x00\x08\x00\x01",
        &[0xFF, 0xD8, 0xFF, 0xE1, 0, 1],
    ] {
        assert_eq!(tiff_date(bytes), None);
        assert_eq!(jpeg_exif(bytes), None);
    }
}

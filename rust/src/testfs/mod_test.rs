#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn a_fixture_reads_as_the_tree_it_describes() {
    let root = new_tree(&[
        ("paris/IMG_1264.RW2", b"raw"),
        ("paris/converted/IMG_1264.jpg", b"jpeg"),
    ]);
    assert_eq!(
        files(&root),
        ["paris/IMG_1264.RW2", "paris/converted/IMG_1264.jpg"]
    );
    assert_eq!(read(&root, "paris/IMG_1264.RW2"), b"raw");
}

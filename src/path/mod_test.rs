#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use vfs::MemoryFS;

fn at(p: &str) -> VfsPath {
    VfsPath::new(MemoryFS::new()).join(p).unwrap()
}

#[test]
fn below_splits_the_components_under_the_base() {
    assert_eq!(
        below(&at("r"), &at("r/2026/x.rw2")),
        Some(vec!["2026", "x.rw2"])
    );
    assert_eq!(
        below(&at(""), &at("2026/x.rw2")),
        Some(vec!["2026", "x.rw2"])
    );
    assert_eq!(below(&at("r"), &at("r")), None);
    assert_eq!(
        below(&at("r"), &at("rr/x")),
        None,
        "a prefix of the name is not inside"
    );
    assert_eq!(below(&at("r"), &at("other/x")), None);
}

#[test]
fn relative_prints_dot_and_dotdot() {
    assert_eq!(relative(&at("r"), &at("r/d/a.png")), "d/a.png");
    assert_eq!(relative(&at("r"), &at("r")), ".");
    assert_eq!(relative(&at(""), &at("paris/a.rw2")), "paris/a.rw2");
    assert_eq!(relative(&at("r/2026"), &at("r/2025/x")), "../2025/x");
}

#[test]
fn stem_and_ext() {
    assert_eq!(stem("IMG_1264.RW2.pp3"), "IMG_1264.RW2");
    assert_eq!(stem("README"), "README");
    assert_eq!(ext("IMG_1264.RW2"), ".rw2");
    assert_eq!(ext("README"), "");
}

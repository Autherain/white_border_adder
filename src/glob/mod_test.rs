#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

fn globs(texts: &[&str]) -> Vec<Glob> {
    texts.iter().map(|t| t.parse().unwrap()).collect()
}

#[test]
fn a_malformed_glob_does_not_parse() {
    assert!("[".parse::<Glob>().is_err());
    assert!("[.rw2".parse::<Glob>().is_err());
    assert!("*.rw2".parse::<Glob>().is_ok());
}

#[test]
fn matching_sees_stem_directories_and_path() {
    let g = globs(&["IMG_12*", "*marie*", "20250804_luchon/_131075*"]);
    let hit = |rel: &str| matching(rel, &g).map(Glob::as_str);

    assert_eq!(hit("paris/IMG_1264.RW2"), Some("IMG_12*"));
    assert_eq!(hit("marie/portrait/IMG_1.RW2"), Some("*marie*"));
    assert_eq!(
        hit("2025/20250804_luchon/_1310752.RW2"),
        None,
        "the path has a year on top"
    );
    assert_eq!(
        hit("20250804_luchon/_1310752.RW2"),
        Some("20250804_luchon/_131075*")
    );
    assert_eq!(hit("paris/IMG_2000.RW2"), None);
}

#[test]
fn matching_ignores_case() {
    let g = globs(&["dsc_*"]);
    assert!(matching("dump/DSC_0001.NEF", &g).is_some());
}

#[test]
fn a_star_stays_inside_one_segment() {
    let g = globs(&["luchon"]);
    assert!(matching("2025/20250804_luchon/_1310726.RW2", &g).is_none());
}

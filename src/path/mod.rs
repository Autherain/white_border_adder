//! Paths the way the user reads them: relative to the argument they typed.
//! `vfs` keeps every path absolute from the filesystem root and `/`-separated,
//! so these are string operations, the same on every OS.

use vfs::VfsPath;

/// The components of `path` under `base`; `None` when `path` is `base` itself
/// or outside it.
pub fn below<'p>(base: &VfsPath, path: &'p VfsPath) -> Option<Vec<&'p str>> {
    let rest = path
        .as_str()
        .strip_prefix(base.as_str())?
        .strip_prefix('/')?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.split('/').collect())
}

/// `path` relative to `base`: "." for the base itself, "../x" for a sibling.
pub fn relative(base: &VfsPath, path: &VfsPath) -> String {
    let from: Vec<&str> = segments(base.as_str());
    let to: Vec<&str> = segments(path.as_str());
    let mut common = 0;
    while common < from.len() && common < to.len() && from[common] == to[common] {
        common += 1;
    }

    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend_from_slice(&to[common..]);
    if parts.is_empty() {
        return ".".to_owned();
    }
    parts.join("/")
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// The name without its last extension: `IMG_1264.RW2.pp3` is `IMG_1264.RW2`.
pub fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) => &name[..dot],
        None => name,
    }
}

/// The extension with its dot, lowercased: ".rw2".
pub fn ext(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) => name[dot..].to_ascii_lowercase(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

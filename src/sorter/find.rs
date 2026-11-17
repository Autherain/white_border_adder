//! The photos under the sources, each with what must follow it if it moves.

use std::collections::HashSet;

use jiff::civil::DateTime;
use vfs::{VfsError, VfsPath};

use super::plan::READERS;
use super::{CONVERTED_DIR, SIDECAR_EXT};
use crate::glob::{self, Glob};
use crate::log::Log;
use crate::path::{relative, stem};
use crate::runeach::run_each;

const PHOTO_EXTS: [&str; 10] = [
    ".rw2", ".jpg", ".jpeg", ".png", ".dng", ".arw", ".cr2", ".cr3", ".nef", ".raf",
];

/// An original found on disk.
pub(super) struct Photo {
    pub(super) path: VfsPath,
    /// Whether a RawTherapee `.pp3` sits beside it.
    pub(super) sidecar: bool,
    /// The files of converted/ made from this photo.
    pub(super) developed: Vec<String>,
}

pub(super) struct Dated {
    pub(super) photo: Photo,
    /// `None` when neither EXIF nor the file gave a date; the photo stays.
    pub(super) when: Option<DateTime>,
}

/// What one directory holds. `scan` runs in the pool, so it hands its
/// findings back instead of writing into a shared list.
struct Scanned {
    photos: Vec<Photo>,
    /// Where to descend next; empty unless --recursive.
    subdirs: Vec<VfsPath>,
}

/// Walks the sources one depth at a time.
pub(super) fn find(
    sources: &[VfsPath],
    recursive: bool,
    exclude: &[Glob],
) -> Result<Vec<Photo>, VfsError> {
    // Two sources can overlap (a directory and one below it); a photo
    // reached twice is planned once.
    let mut seen = HashSet::new();
    let mut photos = Vec::new();
    let mut level = sources.to_vec();

    while !level.is_empty() {
        let scans = scan_level(level, recursive, exclude)?;

        level = Vec::new();
        for scanned in scans {
            level.extend(scanned.subdirs);
            for photo in scanned.photos {
                if seen.insert(photo.path.as_str().to_owned()) {
                    photos.push(photo);
                }
            }
        }
    }
    Ok(photos)
}

/// Scans the directories of one depth at the same time. The first error
/// stops the search.
fn scan_level(
    level: Vec<VfsPath>,
    recursive: bool,
    exclude: &[Glob],
) -> Result<Vec<Scanned>, VfsError> {
    let mut results = Vec::with_capacity(level.len());
    run_each(
        READERS,
        level,
        |dir| scan(&dir, recursive, exclude),
        |result| results.push(result),
    );

    let mut scans = Vec::with_capacity(results.len());
    for result in results {
        scans.push(result?);
    }
    Ok(scans)
}

/// Lists one directory once: its photos, and its subdirectories when recursive.
fn scan(dir: &VfsPath, recursive: bool, exclude: &[Glob]) -> Result<Scanned, VfsError> {
    let entries: Vec<VfsPath> = dir.read_dir()?.collect();
    let names = names_of(&entries);

    let mut converted_files = Vec::new();
    if names.contains(CONVERTED_DIR) {
        converted_files = files_of_converted(dir);
    }

    let mut scanned = Scanned {
        photos: Vec::new(),
        subdirs: Vec::new(),
    };
    for entry in entries {
        let name = entry.filename();
        if entry.is_dir()? {
            if recursive && worth_entering(&name, exclude) {
                scanned.subdirs.push(entry);
            }
            continue;
        }
        if !entry.is_file()? || !is_photo(&name) {
            continue;
        }
        let sidecar = names.contains(&format!("{name}{SIDECAR_EXT}"));
        let developed = developed_from(&name, &converted_files);
        scanned.photos.push(Photo {
            path: entry,
            sidecar,
            developed,
        });
    }
    Ok(scanned)
}

/// A sidecar is looked up by name, once per photo.
fn names_of(entries: &[VfsPath]) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in entries {
        names.insert(entry.filename());
    }
    names
}

/// A directory excluded by its own name is not even listed.
fn worth_entering(name: &str, exclude: &[Glob]) -> bool {
    if holds_no_originals(name) {
        return false;
    }
    glob::matching(name, exclude).is_none()
}

/// converted/ and bordered/ hold what was derived from the originals, not the
/// originals: their dates say nothing and they follow their shoot, so they
/// are never read and never moved. bordered_images is the old script's output.
fn holds_no_originals(name: &str) -> bool {
    name.starts_with('.')
        || name == CONVERTED_DIR
        || name == "bordered"
        || name == "bordered_images"
}

fn is_photo(name: &str) -> bool {
    PHOTO_EXTS.contains(&crate::path::ext(name).as_str())
}

/// The names of the files in `dir`/converted/. A converted/ that cannot be
/// read counts as empty: the photos still move, without developed copies.
fn files_of_converted(dir: &VfsPath) -> Vec<String> {
    let mut names = Vec::new();
    let converted = match dir.join(CONVERTED_DIR) {
        Ok(p) => p,
        Err(_) => return names,
    };
    let entries = match converted.read_dir() {
        Ok(e) => e,
        Err(_) => return names,
    };
    for entry in entries {
        let is_file = match entry.is_file() {
            Ok(b) => b,
            Err(_) => false,
        };
        if is_file {
            names.push(entry.filename());
        }
    }
    names
}

/// The converted files named after the photo: IMG_1264.RW2 has IMG_1264.jpg
/// and IMG_1264-2.png but not IMG_12645.jpg, since the name that follows the
/// stem must not continue it with a digit. Moving a wrong file is worse than
/// missing one, hence stricter than clean's plain prefix.
fn developed_from(photo_name: &str, converted_files: &[String]) -> Vec<String> {
    let stem = stem(photo_name).to_ascii_lowercase();
    let mut developed = Vec::new();
    for file in converted_files {
        let lowercased = file.to_ascii_lowercase();
        let after_stem = match lowercased.strip_prefix(&stem) {
            Some(rest) => rest,
            None => continue,
        };
        if !after_stem.is_empty() && after_stem.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        developed.push(file.clone());
    }
    developed
}

/// Drops the excluded photos before their dates are read: skipping the read
/// is most of the point when the excluded tree is a large archive.
pub(super) fn without_excluded(
    log: &dyn Log,
    photos: Vec<Photo>,
    root: &VfsPath,
    exclude: &[Glob],
) -> Vec<Photo> {
    if exclude.is_empty() {
        return photos;
    }
    let mut remaining = Vec::new();
    for photo in photos {
        // A source outside the root has no path below it; the globs then
        // see its full path.
        let mut rel = relative(root, &photo.path);
        if rel.starts_with("..") {
            rel = photo.path.as_str().to_owned();
        }
        if let Some(glob) = glob::matching(&rel, exclude) {
            log.debug(&format!("skip {rel}: --exclude {glob}"));
            continue;
        }
        remaining.push(photo);
    }
    remaining
}

//! The Instagram copies `wba border` wrote, which --bordered adds to the plan.

use vfs::{VfsError, VfsPath};

use super::Candidate;
use super::rejects::Converted;
use super::scan::{Entry, entries};
use crate::border::{DEFAULT_OUT, LEGACY_OUT, is_image};

/// Where wba border writes, and where the script that preceded it did.
pub(super) fn is_bordered_dir(name: &str) -> bool {
    name == DEFAULT_OUT || name == LEGACY_OUT
}

pub(super) fn bordered_copies(
    entries: &[Entry],
    converted: Option<&Converted>,
) -> Result<Vec<Candidate>, VfsError> {
    let mut copies = Vec::new();
    for dir in bordered_dirs(entries, converted) {
        copies.extend(images_in(&dir)?);
    }
    Ok(copies)
}

/// The bordered folders inside converted/, then those beside it.
fn bordered_dirs(entries: &[Entry], converted: Option<&Converted>) -> Vec<VfsPath> {
    let mut dirs = Vec::new();
    if let Some(converted) = converted {
        dirs.extend_from_slice(&converted.bordered);
    }
    for entry in entries {
        if entry.is_dir() && is_bordered_dir(&entry.name()) {
            dirs.push(entry.path.clone());
        }
    }
    dirs
}

/// Images only, so that a note left in the folder is never deleted.
fn images_in(dir: &VfsPath) -> Result<Vec<Candidate>, VfsError> {
    let mut copies = Vec::new();
    for entry in entries(dir)? {
        if entry.is_file() && is_image(&entry.name()) {
            copies.push(Candidate {
                file: entry.to_file(),
                sidecar: None,
                kept: None,
            });
        }
    }
    Ok(copies)
}

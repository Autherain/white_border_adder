//! The rejects of one directory: the raws with no developed image in
//! converted/.

use std::collections::HashMap;

use vfs::VfsPath;

use super::bordered::is_bordered_dir;
use super::scan::{Entry, entries};
use super::{CONVERTED_DIR, Candidate, File, Options, SIDECAR_EXT};
use crate::path::stem;

pub(super) struct Converted {
    /// The lowercased names of its regular files.
    pub(super) names: Vec<String>,
    /// The bordered folders inside it: border enters converted/, so its
    /// output lands there as often as beside it.
    pub(super) bordered: Vec<VfsPath>,
}

/// What converted/ holds; `None` when the listing has no such directory or it
/// cannot be read, which `scan` treats as "not a sorted shoot".
pub(super) fn read_converted(parent: &[Entry]) -> Option<Converted> {
    let dir = converted_dir(parent)?;
    let inside = match entries(&dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    let mut converted = Converted {
        names: Vec::new(),
        bordered: Vec::new(),
    };
    for entry in inside {
        if entry.is_file() {
            converted.names.push(entry.name().to_ascii_lowercase());
        } else if entry.is_dir() && is_bordered_dir(&entry.name()) {
            converted.bordered.push(entry.path);
        }
    }
    Some(converted)
}

fn converted_dir(parent: &[Entry]) -> Option<VfsPath> {
    for entry in parent {
        if entry.is_dir() && entry.name() == CONVERTED_DIR {
            return Some(entry.path.clone());
        }
    }
    None
}

/// Each raw matching the pattern that was never developed, with its sidecar
/// when it has one.
pub(super) fn undeveloped_raws(
    entries: &[Entry],
    developed: &[String],
    options: &Options,
) -> Vec<Candidate> {
    let sidecars = sidecars_by_name(entries);

    let mut rejects = Vec::new();
    for entry in entries {
        let name = entry.name();
        if !entry.is_file() || !options.pattern.matches(&name) {
            continue;
        }
        if is_developed(&name, developed) {
            continue;
        }
        rejects.push(Candidate {
            file: entry.to_file(),
            sidecar: sidecar_of(&name, &sidecars),
            kept: None,
        });
    }
    rejects
}

/// RawTherapee names its output after the raw and may add a suffix
/// (IMG_1265.RW2 gives IMG_1265_converted.jpg), hence the prefix test.
fn is_developed(raw_name: &str, developed: &[String]) -> bool {
    let stem = stem(raw_name).to_ascii_lowercase();
    for name in developed {
        if name.starts_with(&stem) {
            return true;
        }
    }
    false
}

fn sidecars_by_name(entries: &[Entry]) -> HashMap<String, File> {
    let mut sidecars = HashMap::new();
    for entry in entries {
        if entry.is_file() && entry.name().ends_with(SIDECAR_EXT) {
            sidecars.insert(entry.name(), entry.to_file());
        }
    }
    sidecars
}

/// The sidecar is named after the whole raw: IMG_1265.RW2.pp3.
fn sidecar_of(raw_name: &str, sidecars: &HashMap<String, File>) -> Option<File> {
    match sidecars.get(&format!("{raw_name}{SIDECAR_EXT}")) {
        Some(file) => Some(file.clone()),
        None => None,
    }
}

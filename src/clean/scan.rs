//! One directory, listed once: everything clean wants from it comes from
//! that listing.

use vfs::{VfsError, VfsFileType, VfsMetadata, VfsPath};

use super::bordered::{bordered_copies, is_bordered_dir};
use super::rejects::{read_converted, undeveloped_raws};
use super::{CONVERTED_DIR, Candidate, File, Options, Skipped};

/// What one directory adds to the plan. `scan` runs in the pool, so it hands
/// its findings back instead of writing into a shared `Plan`.
pub(super) struct Scanned {
    pub(super) dir: VfsPath,
    pub(super) candidates: Vec<Candidate>,
    /// Where to descend next; empty unless --recursive.
    pub(super) subdirs: Vec<VfsPath>,
    /// Set when the directory has no converted/: its raws were left alone.
    pub(super) skipped: Option<Skipped>,
}

/// A directory entry with its metadata, read once: `scan` goes over the same
/// listing several times and each read is a round trip on a slow mount.
pub(super) struct Entry {
    pub(super) path: VfsPath,
    pub(super) meta: VfsMetadata,
}

impl Entry {
    pub(super) fn is_file(&self) -> bool {
        self.meta.file_type == VfsFileType::File
    }

    pub(super) fn is_dir(&self) -> bool {
        self.meta.file_type == VfsFileType::Directory
    }

    pub(super) fn name(&self) -> String {
        self.path.filename()
    }

    pub(super) fn to_file(&self) -> File {
        File {
            path: self.path.clone(),
            size: self.meta.len,
        }
    }
}

pub(super) fn entries(dir: &VfsPath) -> Result<Vec<Entry>, VfsError> {
    let mut out = Vec::new();
    for path in dir.read_dir()? {
        let meta = path.metadata()?;
        out.push(Entry { path, meta });
    }
    Ok(out)
}

pub(super) fn scan(dir: &VfsPath, options: &Options) -> Result<Scanned, VfsError> {
    let entries = entries(dir)?;
    let converted = read_converted(&entries);

    let mut candidates = Vec::new();
    let mut skipped = None;
    // Without converted/ nothing says which raws were developed, so none can
    // be called a reject: the directory is only counted, for the recap.
    match &converted {
        Some(converted) => {
            candidates = undeveloped_raws(&entries, &converted.names, options);
        }
        None => skipped = Some(count_raws(dir, &entries, options)),
    }

    if options.bordered {
        candidates.extend(bordered_copies(&entries, converted.as_ref())?);
    }

    let mut subdirs = Vec::new();
    if options.recursive {
        subdirs = subdirs_to_visit(&entries);
    }

    Ok(Scanned {
        dir: dir.clone(),
        candidates,
        subdirs,
        skipped,
    })
}

/// The raws clean will not touch, so the recap can say what sits there unsorted.
fn count_raws(dir: &VfsPath, entries: &[Entry], options: &Options) -> Skipped {
    let mut skipped = Skipped {
        dir: dir.clone(),
        raws: 0,
        size: 0,
    };
    for entry in entries {
        if entry.is_file() && options.pattern.matches(&entry.name()) {
            skipped.raws += 1;
            skipped.size += entry.meta.len;
        }
    }
    skipped
}

fn subdirs_to_visit(entries: &[Entry]) -> Vec<VfsPath> {
    let mut subdirs = Vec::new();
    for entry in entries {
        if entry.is_dir() && !holds_no_raws(&entry.name()) {
            subdirs.push(entry.path.clone());
        }
    }
    subdirs
}

/// Hidden directories, RawTherapee's output and wba's own.
fn holds_no_raws(name: &str) -> bool {
    name.starts_with('.') || name == CONVERTED_DIR || is_bordered_dir(name)
}

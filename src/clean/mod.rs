//! Raws that RawTherapee never developed, and the sidecars that go with them.

mod plan;

use std::fmt;

use vfs::{VfsError, VfsPath};

use crate::glob::Glob;

pub use plan::plan;

pub const CONVERTED_DIR: &str = "converted";
pub const SIDECAR_EXT: &str = ".pp3";

pub struct Options {
    pub pattern: Glob,
    /// Exclude globs keep what they match; Only globs, when given, keep
    /// everything they do not match. Both see the stem, each directory of
    /// the path below the argument, and the whole path.
    pub exclude: Vec<Glob>,
    pub only: Vec<Glob>,
    pub recursive: bool,
    /// Adds the white-bordered Instagram copies to the plan.
    pub bordered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: VfsPath,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub raw: File,
    pub sidecar: Option<File>,
    /// Why this candidate stays. Kept candidates stay in the plan so the
    /// caller can say what was spared.
    pub kept: Option<Kept>,
    /// An Instagram copy rather than a raw; it has no sidecar.
    pub bordered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    Excluded(Glob),
    NotInOnly(Vec<Glob>),
}

impl fmt::Display for Kept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Excluded(g) => write!(f, "--exclude {g}"),
            Self::NotInOnly(globs) => {
                f.write_str("not in --only")?;
                globs.iter().try_for_each(|g| write!(f, " {g}"))
            }
        }
    }
}

/// A directory that holds raws but no converted/, so no sorting happened
/// there and clean leaves it alone; the raw count says what is waiting in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub dir: VfsPath,
    pub raws: usize,
    pub size: u64,
}

/// The plan: what would go or is kept, and what was not even looked at.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub candidates: Vec<Candidate>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, thiserror::Error)]
#[error("{}: {source}", path.as_str())]
pub struct Failed {
    pub path: VfsPath,
    #[source]
    pub source: VfsError,
}

impl Candidate {
    /// The raw, then the sidecar when present: what `delete` removes.
    pub fn files(&self) -> Vec<&File> {
        let mut files = vec![&self.raw];
        if let Some(sidecar) = &self.sidecar {
            files.push(sidecar);
        }
        files
    }

    pub fn size(&self) -> u64 {
        let mut total = 0;
        for f in self.files() {
            total += f.size;
        }
        total
    }

    /// Removes the raw and its sidecar. The candidate is consumed: a deleted
    /// file cannot be deleted twice. The paths removed come back for the
    /// caller to print; the first failure stops the rest.
    pub fn delete(self) -> Result<Vec<VfsPath>, Failed> {
        let mut files = Vec::new();
        for f in self.files() {
            files.push(f.path.clone());
        }
        for path in &files {
            if let Err(source) = path.remove_file() {
                return Err(Failed {
                    path: path.clone(),
                    source,
                });
            }
        }
        if self.bordered {
            remove_if_empty(&self.raw.path.parent());
        }
        Ok(files)
    }
}

/// Tidies the bordered/ folder once its last copy is gone; what is left in
/// it (a file kept by --exclude, a note of yours) keeps it alive.
fn remove_if_empty(dir: &VfsPath) {
    let mut entries = match dir.read_dir() {
        Ok(e) => e,
        Err(_) => return,
    };
    if entries.next().is_none() {
        let _ = dir.remove_dir();
    }
}

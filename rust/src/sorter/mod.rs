//! Photos filed under <year>/<day folder> by the date they were shot.

mod exif;
mod plan;

use std::fmt;

use jiff::civil::DateTime;
use vfs::{VfsError, VfsPath};

use crate::glob::Glob;

pub use plan::plan;

pub const SIDECAR_EXT: &str = ".pp3";
pub const CONVERTED_DIR: &str = "converted";

/// The old Nikon photos: their folders and numbering are already the way the
/// owner wants them, and no run may move them.
pub const ALWAYS_EXCLUDED: [&str; 2] = ["DSC_*", "*nikon*"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    pub recursive: bool,
    /// Exclude globs leave a photo, and any directory they match, untouched
    /// and unread. They see the stem, each directory of the path below the
    /// root and the whole path, as in clean.
    pub exclude: Vec<Glob>,
}

/// Where a photo's date comes from. The real one reads EXIF and falls back
/// to the modification time; a test can answer anything.
#[cfg_attr(test, mockall::automock)]
pub trait Dates: Sync {
    fn when(&self, path: &VfsPath) -> Option<DateTime>;
}

/// The moment the camera recorded, read from the head of the file only, or
/// the modification time when there is none (a PNG, for instance).
pub struct ExifOrMtime;

impl Dates for ExifOrMtime {
    fn when(&self, path: &VfsPath) -> Option<DateTime> {
        if let Some(shot) = exif::shot_date(path) {
            return Some(shot);
        }
        let Ok(meta) = path.metadata() else {
            return None;
        };
        let modified = meta.modified?;
        let Ok(stamp) = jiff::Timestamp::try_from(modified) else {
            return None;
        };
        Some(stamp.to_zoned(jiff::tz::TimeZone::system()).datetime())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub from: VfsPath,
    pub to: VfsPath,
}

/// Sends one photo to <root>/<year>/<day folder>/ with what belongs to it:
/// its RawTherapee sidecar and its developed copies from converted/.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: VfsPath,
    /// `None` when no target could be chosen; then `kept` says why.
    pub to: Option<VfsPath>,
    pub with: Vec<Pair>,
    /// Why the photo stays where it is; kept moves stay in the plan so the
    /// recap can show what was left alone.
    pub kept: Option<Kept>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    NoDate,
    SeveralFolders {
        day: String,
        year: String,
        folders: Vec<String>,
    },
    SameNameAs(VfsPath),
    AlreadyIn(String),
}

impl fmt::Display for Kept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDate => f.write_str("no date"),
            Self::SeveralFolders { day, year, folders } => {
                write!(
                    f,
                    "{} folders for {day} in {year}/: {}",
                    folders.len(),
                    folders.join(", ")
                )
            }
            Self::SameNameAs(other) => write!(f, "same name as {}", other.as_str()),
            Self::AlreadyIn(folder) => write!(f, "already exists in {folder}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: already exists")]
    Exists(String),
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Fs(Vec<VfsError>),
}

#[derive(Debug, thiserror::Error)]
#[error("{}: {source}", from.as_str())]
pub struct Failed {
    pub from: VfsPath,
    #[source]
    pub source: Error,
}

impl Move {
    /// Every file that moves, the photo first; nothing for a move without a target.
    pub fn pairs(&self) -> Vec<Pair> {
        let Some(to) = &self.to else {
            return Vec::new();
        };
        let photo = Pair {
            from: self.from.clone(),
            to: to.clone(),
        };
        std::iter::once(photo)
            .chain(self.with.iter().cloned())
            .collect()
    }

    /// Checks every target again right before moving: a rename onto an
    /// empty directory or a stale file must never replace something. The
    /// photo goes first, so a failure never leaves developed copies without
    /// their original. The move is consumed; the pairs moved come back for
    /// the caller to print.
    pub fn run(self) -> Result<Vec<Pair>, Failed> {
        let pairs = self.pairs();
        match run(&pairs) {
            Ok(()) => Ok(pairs),
            Err(source) => Err(Failed {
                from: self.from,
                source,
            }),
        }
    }
}

fn run(pairs: &[Pair]) -> Result<(), Error> {
    if let Some(taken) = pairs.iter().find(|p| p.to.exists().unwrap_or(true)) {
        return Err(Error::Exists(taken.to.as_str().to_owned()));
    }

    let mut errs = Vec::new();
    for (i, pair) in pairs.iter().enumerate() {
        let mut moved = pair.to.parent().create_dir_all();
        if moved.is_ok() {
            moved = pair.from.move_file(&pair.to);
        }
        if let Err(e) = moved {
            if i == 0 {
                return Err(Error::Fs(vec![e]));
            }
            errs.push(e);
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(Error::Fs(errs))
    }
}

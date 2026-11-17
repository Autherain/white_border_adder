use vfs::{VfsError, VfsPath};

use super::day::is_year;
use super::find::{Dated, Photo, find, without_excluded};
use super::planner::Planner;
use super::{ALWAYS_EXCLUDED, Dates, Move, Options};
use crate::glob::Glob;
use crate::log::Log;
use crate::runeach::run_each;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: not a directory")]
    NotADirectory(String),
    #[error(transparent)]
    Fs(#[from] VfsError),
}

/// IO bound, not CPU bound: each task is one small read from a possibly
/// remote drive, so far more than the CPU count run at once.
pub(super) const READERS: usize = 32;

/// Files every photo found under `sources` (the root itself when none) under
/// <root>/<year>/<day folder> by the date `dates` gives. The day folder is, in
/// order: the dated folder the photo already sits in when its date is right;
/// the one existing under the year for that day, so the place name survives;
/// else the bare date.
pub fn plan(
    log: &dyn Log,
    dates: &dyn Dates,
    root: &VfsPath,
    sources: &[VfsPath],
    options: &Options,
) -> Result<Vec<Move>, Error> {
    check_directories(root, sources)?;
    let scope = scope_of(root, sources)?;
    let exclude = with_always_excluded(&options.exclude);

    let photos = find(&scope.sources, options.recursive, &exclude)?;
    let photos = without_excluded(log, photos, &scope.root, &exclude);
    let dated_photos = read_dates(dates, photos);
    Ok(decide_all(scope.root, dated_photos))
}

fn check_directories(root: &VfsPath, sources: &[VfsPath]) -> Result<(), Error> {
    if !root.is_dir()? {
        return Err(Error::NotADirectory(root.as_str().to_owned()));
    }
    for source in sources {
        if !source.is_dir()? {
            return Err(Error::NotADirectory(source.as_str().to_owned()));
        }
    }
    Ok(())
}

/// Where the year folders live, and where to look for photos.
struct Scope {
    root: VfsPath,
    sources: Vec<VfsPath>,
}

/// Without a source the root itself is searched.
fn scope_of(root: &VfsPath, sources: &[VfsPath]) -> Result<Scope, VfsError> {
    if !sources.is_empty() {
        return Ok(Scope {
            root: root.clone(),
            sources: sources.to_vec(),
        });
    }

    let mut scope = Scope {
        root: root.clone(),
        sources: vec![root.clone()],
    };
    // Run from inside 2026/ the year folders live one level up; without
    // this a 2025 photo would land in 2026/2025/.
    if is_year(&root.filename()) {
        scope.root = root.join("..")?;
    }
    Ok(scope)
}

fn with_always_excluded(from_flags: &[Glob]) -> Vec<Glob> {
    let mut exclude = from_flags.to_vec();
    for text in ALWAYS_EXCLUDED {
        if let Ok(glob) = text.parse() {
            exclude.push(glob);
        }
    }
    exclude
}

fn read_dates(dates: &dyn Dates, photos: Vec<Photo>) -> Vec<Dated> {
    let mut dated_photos = Vec::with_capacity(photos.len());
    run_each(
        READERS,
        photos,
        |photo| read_date(dates, photo),
        |dated| dated_photos.push(dated),
    );
    // Completion order is arbitrary; the first of two same-named photos
    // must be the same on every run.
    dated_photos.sort_by(|a, b| a.photo.path.as_str().cmp(b.photo.path.as_str()));
    dated_photos
}

fn read_date(dates: &dyn Dates, photo: Photo) -> Dated {
    let when = dates.when(&photo.path);
    Dated { photo, when }
}

fn decide_all(root: VfsPath, dated_photos: Vec<Dated>) -> Vec<Move> {
    let mut planner = Planner::new(root);
    let mut moves = Vec::new();
    for dated in dated_photos {
        if let Some(planned) = planner.decide(dated) {
            moves.push(planned);
        }
    }
    moves.sort_by(|a, b| a.from.as_str().cmp(b.from.as_str()));
    moves
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;

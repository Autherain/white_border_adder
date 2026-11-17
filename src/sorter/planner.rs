//! Where one photo goes, given the ones decided before it.

use std::collections::HashMap;

use vfs::VfsPath;

use super::day::{Day, Place, already_filed, dated_folder_above};
use super::find::{Dated, Photo};
use super::{CONVERTED_DIR, Kept, Move, Pair, SIDECAR_EXT};
use crate::path::below;

/// Decides the photos one after the other: a decision depends on the ones
/// before it, through `taken`.
pub(super) struct Planner {
    root: VfsPath,
    /// Year directory to the names of its subdirectories, listed once.
    dated_folders: HashMap<String, Vec<String>>,
    /// Planned destination to its source, so two cards with an IMG_0001.JPG
    /// do not collide.
    taken: HashMap<String, VfsPath>,
}

impl Planner {
    pub(super) fn new(root: VfsPath) -> Self {
        Self {
            root,
            dated_folders: HashMap::new(),
            taken: HashMap::new(),
        }
    }

    /// `None` when the photo stays where it is and nothing is listed.
    pub(super) fn decide(&mut self, dated: Dated) -> Option<Move> {
        let from = dated.photo.path.clone();
        let when = match dated.when {
            Some(w) => w,
            None => return Some(stays(from, Kept::NoDate)),
        };
        let day = Day::of(when);

        let parent = from.parent();
        let folders = match below(&self.root, &parent) {
            Some(f) => f,
            None => Vec::new(),
        };
        if already_filed(&folders, &day) {
            return None;
        }

        let place = match self.place(&folders, &day) {
            Ok(p) => p,
            Err(why) => return Some(stays(from, why)),
        };
        let to_dir = self.dir_of(&day.year, &place)?;
        if to_dir == parent {
            return None;
        }
        Some(self.move_to(dated.photo, &to_dir))
    }

    fn place(&mut self, folders: &[String], day: &Day) -> Result<Place, Kept> {
        if let Some(place) = dated_folder_above(folders, day) {
            return Ok(place);
        }
        let day_folder = self.day_folder(day)?;
        Ok(Place {
            day_folder,
            below: Vec::new(),
        })
    }

    /// <root>/<year>/<day folder>/<below...>; `None` when a name cannot be
    /// joined, and the photo then stays unlisted.
    fn dir_of(&self, year: &str, place: &Place) -> Option<VfsPath> {
        let mut names = vec![year.to_owned(), place.day_folder.clone()];
        names.extend_from_slice(&place.below);

        let mut dir = self.root.clone();
        for name in names {
            dir = match dir.join(&name) {
                Ok(d) => d,
                Err(_) => return None,
            };
        }
        Some(dir)
    }

    /// The folder for `day` under its year: the one that exists for that
    /// day, else the bare date. Two existing folders for one day is a choice
    /// wba does not make, and the photo stays.
    fn day_folder(&mut self, day: &Day) -> Result<String, Kept> {
        let year_dir = match self.root.join(&day.year) {
            Ok(d) => d,
            Err(_) => return Err(Kept::NoDate),
        };

        let mut found = Vec::new();
        for folder in self.folders_of(&year_dir) {
            if day.names(&folder) {
                found.push(folder);
            }
        }
        // A listing comes in no particular order; the reason must read the
        // same every run.
        found.sort();

        match found.len() {
            0 => Ok(day.compact.clone()),
            1 => Ok(found.remove(0)),
            _ => Err(Kept::SeveralFolders {
                day: day.compact.clone(),
                year: day.year.clone(),
                folders: found,
            }),
        }
    }

    /// The subdirectories of a year directory, listed on first use: every
    /// photo of that year asks.
    fn folders_of(&mut self, year_dir: &VfsPath) -> Vec<String> {
        if let Some(known) = self.dated_folders.get(year_dir.as_str()) {
            return known.clone();
        }
        let names = subdirectories(year_dir);
        self.dated_folders
            .insert(year_dir.as_str().to_owned(), names.clone());
        names
    }

    /// The move of `photo` and its companions into `to_dir`, kept when one
    /// of the targets is already planned or already on disk.
    fn move_to(&mut self, photo: Photo, to_dir: &VfsPath) -> Move {
        let with = companions(&photo, to_dir);
        let from = photo.path;
        let to = match to_dir.join(from.filename()) {
            Ok(t) => t,
            Err(_) => return stays(from, Kept::NoDate),
        };

        let mut planned = Move {
            from,
            to: Some(to),
            with,
            kept: None,
        };
        planned.kept = self.conflict(&planned, to_dir);
        // Only a move that will run reserves its targets.
        if planned.kept.is_none() {
            self.reserve(&planned);
        }
        planned
    }

    fn conflict(&self, planned: &Move, to_dir: &VfsPath) -> Option<Kept> {
        for pair in planned.pairs() {
            if let Some(other) = self.taken.get(pair.to.as_str()) {
                return Some(Kept::SameNameAs(other.clone()));
            }
            // A target that cannot be checked counts as taken.
            let exists = match pair.to.exists() {
                Ok(b) => b,
                Err(_) => true,
            };
            if exists {
                return Some(Kept::AlreadyIn(to_dir.filename()));
            }
        }
        None
    }

    fn reserve(&mut self, planned: &Move) {
        for pair in planned.pairs() {
            self.taken.insert(pair.to.as_str().to_owned(), pair.from);
        }
    }
}

/// A directory that does not exist yet, or cannot be read, has none.
fn subdirectories(dir: &VfsPath) -> Vec<String> {
    let mut names = Vec::new();
    let entries = match dir.read_dir() {
        Ok(e) => e,
        Err(_) => return names,
    };
    for entry in entries {
        let is_dir = match entry.is_dir() {
            Ok(b) => b,
            Err(_) => false,
        };
        if is_dir {
            names.push(entry.filename());
        }
    }
    names
}

fn stays(from: VfsPath, why: Kept) -> Move {
    Move {
        from,
        to: None,
        with: Vec::new(),
        kept: Some(why),
    }
}

/// What follows the photo keeps its name and its place beside it: the
/// sidecar next to it, the developed copies in converted/.
fn companions(photo: &Photo, to_dir: &VfsPath) -> Vec<Pair> {
    let mut names = Vec::new();
    if photo.sidecar {
        names.push(format!("{}{SIDECAR_EXT}", photo.path.filename()));
    }
    for name in &photo.developed {
        names.push(format!("{CONVERTED_DIR}/{name}"));
    }

    let mut pairs = Vec::new();
    for name in names {
        if let Some(pair) = pair(&photo.path.parent(), to_dir, &name) {
            pairs.push(pair);
        }
    }
    pairs
}

/// The same name under two directories; `None` when it cannot be joined
/// to one of them, and the file then stays behind.
fn pair(from_dir: &VfsPath, to_dir: &VfsPath, name: &str) -> Option<Pair> {
    let from = match from_dir.join(name) {
        Ok(p) => p,
        Err(_) => return None,
    };
    let to = match to_dir.join(name) {
        Ok(p) => p,
        Err(_) => return None,
    };
    Some(Pair { from, to })
}

use std::collections::{HashMap, HashSet};

use jiff::civil::DateTime;
use vfs::{VfsError, VfsPath};

use super::{ALWAYS_EXCLUDED, CONVERTED_DIR, Dates, Kept, Move, Options, Pair, SIDECAR_EXT};
use crate::glob::{self, Glob};
use crate::log::Log;
use crate::path::{below, relative, stem};
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
const READERS: usize = 32;

const PHOTO_EXTS: [&str; 10] = [
    ".rw2", ".jpg", ".jpeg", ".png", ".dng", ".arw", ".cr2", ".cr3", ".nef", ".raf",
];

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
    o: &Options,
) -> Result<Vec<Move>, Error> {
    if !root.is_dir()? {
        return Err(Error::NotADirectory(root.as_str().to_owned()));
    }
    for source in sources {
        if !source.is_dir()? {
            return Err(Error::NotADirectory(source.as_str().to_owned()));
        }
    }

    let mut root = root.clone();
    let mut sources = sources.to_vec();
    if sources.is_empty() {
        sources.push(root.clone());
        // Run from inside 2026/ the year folders live one level up; without
        // this a 2025 photo would land in 2026/2025/.
        if is_year(&root.filename()) {
            root = root.join("..")?;
        }
    }

    let mut exclude = o.exclude.clone();
    for text in ALWAYS_EXCLUDED {
        if let Ok(g) = text.parse() {
            exclude.push(g);
        }
    }

    let photos = find(&sources, o.recursive, &exclude)?;
    let photos = left_out(log, photos, &root, &exclude);

    let mut read = Vec::with_capacity(photos.len());
    run_each(READERS, photos, |ph| read_date(dates, ph), |d| read.push(d));
    // Completion order is arbitrary; the first of two same-named photos
    // must be the same on every run.
    read.sort_by(|a, b| a.photo.path.as_str().cmp(b.photo.path.as_str()));

    let mut p = Planner {
        root: &root,
        listed: HashMap::new(),
        taken: HashMap::new(),
    };
    let mut moves = Vec::new();
    for d in read {
        if let Some(m) = p.decide(d) {
            moves.push(m);
        }
    }
    moves.sort_by(|a, b| a.from.as_str().cmp(b.from.as_str()));
    Ok(moves)
}

fn read_date(dates: &dyn Dates, photo: Photo) -> Dated {
    let when = dates.when(&photo.path);
    Dated { photo, when }
}

fn is_year(name: &str) -> bool {
    name.len() == 4 && name.bytes().all(|b| b.is_ascii_digit())
}

/// converted/ and bordered/ hold what was derived from the originals, not the
/// originals: their dates say nothing and they follow their shoot, so they
/// are never read and never moved. bordered_images is the old script's output.
fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') || name == "converted" || name == "bordered" || name == "bordered_images"
}

struct Photo {
    path: VfsPath,
    sidecar: bool,
    /// The files of converted/ made from this photo.
    developed: Vec<String>,
}

struct Dated {
    photo: Photo,
    when: Option<DateTime>,
}

struct Scanned {
    photos: Vec<Photo>,
    subdirs: Vec<VfsPath>,
}

fn find(sources: &[VfsPath], recursive: bool, exclude: &[Glob]) -> Result<Vec<Photo>, VfsError> {
    let mut seen = HashSet::new();
    let mut photos = Vec::new();
    let mut level = sources.to_vec();

    while !level.is_empty() {
        let mut got = Vec::with_capacity(level.len());
        run_each(
            READERS,
            level,
            |dir| scan(&dir, recursive, exclude),
            |s| got.push(s),
        );

        level = Vec::new();
        for s in got {
            let s = s?;
            for ph in s.photos {
                if seen.insert(ph.path.as_str().to_owned()) {
                    photos.push(ph);
                }
            }
            level.extend(s.subdirs);
        }
    }
    Ok(photos)
}

fn scan(dir: &VfsPath, recursive: bool, exclude: &[Glob]) -> Result<Scanned, VfsError> {
    let entries: Vec<VfsPath> = dir.read_dir()?.collect();

    let mut names = HashSet::new();
    for e in &entries {
        names.insert(e.filename());
    }

    let mut converted = Vec::new();
    if names.contains(CONVERTED_DIR) {
        converted = developed_names(dir);
    }

    let mut got = Scanned {
        photos: Vec::new(),
        subdirs: Vec::new(),
    };
    for e in entries {
        let name = e.filename();
        if e.is_dir()? {
            // A directory excluded by its own name is not even listed.
            if recursive && !skipped_dir(&name) && glob::matching(&name, exclude).is_none() {
                got.subdirs.push(e);
            }
            continue;
        }
        if !e.is_file()? || !PHOTO_EXTS.contains(&crate::path::ext(&name).as_str()) {
            continue;
        }
        let sidecar = names.contains(&format!("{name}{SIDECAR_EXT}"));
        let developed = developed_from(&name, &converted);
        got.photos.push(Photo {
            path: e,
            sidecar,
            developed,
        });
    }
    Ok(got)
}

fn developed_names(dir: &VfsPath) -> Vec<String> {
    let mut names = Vec::new();
    let converted = match dir.join(CONVERTED_DIR) {
        Ok(p) => p,
        Err(_) => return names,
    };
    let entries = match converted.read_dir() {
        Ok(e) => e,
        Err(_) => return names,
    };
    for e in entries {
        let is_file = match e.is_file() {
            Ok(b) => b,
            Err(_) => false,
        };
        if is_file {
            names.push(e.filename());
        }
    }
    names
}

/// The converted files named after the photo: IMG_1264.RW2 has IMG_1264.jpg
/// and IMG_1264-2.png but not IMG_12645.jpg, since the name that follows the
/// stem must not continue it with a digit. Moving a wrong file is worse than
/// missing one, hence stricter than clean's plain prefix.
fn developed_from(name: &str, converted: &[String]) -> Vec<String> {
    let stem = stem(name).to_ascii_lowercase();
    let mut out = Vec::new();
    for c in converted {
        let low = c.to_ascii_lowercase();
        let rest = match low.strip_prefix(&stem) {
            Some(r) => r,
            None => continue,
        };
        if !rest.is_empty() && rest.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        out.push(c.clone());
    }
    out
}

/// Drops the excluded photos before their dates are read: skipping the read
/// is most of the point when the excluded tree is a large archive.
fn left_out(log: &dyn Log, photos: Vec<Photo>, root: &VfsPath, exclude: &[Glob]) -> Vec<Photo> {
    if exclude.is_empty() {
        return photos;
    }
    let mut kept = Vec::new();
    for ph in photos {
        let mut rel = relative(root, &ph.path);
        if rel.starts_with("..") {
            rel = ph.path.as_str().to_owned();
        }
        if let Some(g) = glob::matching(&rel, exclude) {
            log.debug(&format!("skip {rel}: --exclude {g}"));
            continue;
        }
        kept.push(ph);
    }
    kept
}

struct Planner<'a> {
    root: &'a VfsPath,
    /// The dated folders of each year directory, listed once.
    listed: HashMap<String, Vec<String>>,
    /// Planned destination to its source, so two cards with an IMG_0001.JPG
    /// do not collide.
    taken: HashMap<String, VfsPath>,
}

fn kept(from: VfsPath, why: Kept) -> Move {
    Move {
        from,
        to: None,
        with: Vec::new(),
        kept: Some(why),
    }
}

impl Planner<'_> {
    /// `None` when the photo stays where it is and nothing is listed.
    fn decide(&mut self, d: Dated) -> Option<Move> {
        let from = d.photo.path.clone();
        let when = match d.when {
            Some(w) => w,
            None => return Some(kept(from, Kept::NoDate)),
        };

        let day = when.strftime("%Y%m%d").to_string();
        let iso = when.strftime("%Y-%m-%d").to_string();
        let year = when.strftime("%Y").to_string();
        let dated = |name: &str| name.starts_with(&day) || name.starts_with(&iso);

        let parent = from.parent();
        let parts = match below(self.root, &parent) {
            Some(p) => p,
            None => Vec::new(),
        };

        if parts.len() >= 2 && parts[0] == year && dated(parts[1]) {
            return None;
        }

        // The dated folder the photo already sits in, at the top or just
        // under a year folder, keeps its name; what is below it follows.
        let mut name = String::new();
        let mut sub: &[&str] = &[];
        for i in 0..parts.len().min(2) {
            if dated(parts[i]) && (i == 0 || is_year(parts[0])) {
                name = parts[i].to_owned();
                sub = &parts[i + 1..];
                break;
            }
        }

        if name.is_empty() {
            match self.day_folder(&year, &day, &dated) {
                Ok(found) => name = found,
                Err(why) => return Some(kept(from, why)),
            }
        }

        let mut dir = match self.root.join(&year) {
            Ok(d) => d,
            Err(_) => return None,
        };
        dir = match dir.join(&name) {
            Ok(d) => d,
            Err(_) => return None,
        };
        for s in sub {
            dir = match dir.join(s) {
                Ok(d) => d,
                Err(_) => return None,
            };
        }
        if dir == parent {
            return None;
        }

        Some(self.target(d.photo, &dir))
    }

    fn day_folder(
        &mut self,
        year: &str,
        day: &str,
        dated: &dyn Fn(&str) -> bool,
    ) -> Result<String, Kept> {
        let dir = match self.root.join(year) {
            Ok(d) => d,
            Err(_) => return Err(Kept::NoDate),
        };

        if !self.listed.contains_key(dir.as_str()) {
            let mut names = Vec::new();
            if let Ok(entries) = dir.read_dir() {
                for e in entries {
                    let is_dir = match e.is_dir() {
                        Ok(b) => b,
                        Err(_) => false,
                    };
                    if is_dir {
                        names.push(e.filename());
                    }
                }
            }
            self.listed.insert(dir.as_str().to_owned(), names);
        }

        let mut found = Vec::new();
        for n in &self.listed[dir.as_str()] {
            if dated(n) {
                found.push(n.clone());
            }
        }
        // A listing comes in no particular order; the reason must read the same every run.
        found.sort();

        match found.len() {
            0 => Ok(day.to_owned()),
            1 => Ok(found.remove(0)),
            _ => Err(Kept::SeveralFolders {
                day: day.to_owned(),
                year: year.to_owned(),
                folders: found,
            }),
        }
    }

    fn target(&mut self, photo: Photo, dir: &VfsPath) -> Move {
        let from = photo.path;
        let to = match dir.join(from.filename()) {
            Ok(t) => t,
            Err(_) => return kept(from, Kept::NoDate),
        };

        let mut with = Vec::new();
        if photo.sidecar {
            let sidecar_from = from
                .parent()
                .join(format!("{}{SIDECAR_EXT}", from.filename()));
            let sidecar_to = dir.join(format!("{}{SIDECAR_EXT}", to.filename()));
            if let (Ok(from), Ok(to)) = (sidecar_from, sidecar_to) {
                with.push(Pair { from, to });
            }
        }
        for name in &photo.developed {
            let developed_from = from.parent().join(format!("{CONVERTED_DIR}/{name}"));
            let developed_to = dir.join(format!("{CONVERTED_DIR}/{name}"));
            if let (Ok(from), Ok(to)) = (developed_from, developed_to) {
                with.push(Pair { from, to });
            }
        }

        let mut m = Move {
            from,
            to: Some(to),
            with,
            kept: None,
        };
        for pair in m.pairs() {
            if let Some(other) = self.taken.get(pair.to.as_str()) {
                m.kept = Some(Kept::SameNameAs(other.clone()));
                return m;
            }
            // A target that cannot be checked counts as taken.
            let exists = match pair.to.exists() {
                Ok(b) => b,
                Err(_) => true,
            };
            if exists {
                m.kept = Some(Kept::AlreadyIn(dir.filename()));
                return m;
            }
        }

        for pair in m.pairs() {
            self.taken.insert(pair.to.as_str().to_owned(), pair.from);
        }
        m
    }
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;

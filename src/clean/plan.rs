use std::collections::HashMap;

use vfs::{VfsError, VfsFileType, VfsMetadata, VfsPath};

use super::{CONVERTED_DIR, Candidate, File, Kept, Options, Plan, SIDECAR_EXT, Skipped};
use crate::border::{DEFAULT_OUT, LEGACY_OUT, is_image};
use crate::glob;
use crate::log::Log;
use crate::path::{relative, stem};
use crate::runeach::run_each;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: not a directory")]
    NotADirectory(String),
    #[error(transparent)]
    Fs(#[from] VfsError),
}

/// A slow mount (a cloud drive over 9p) makes every listing a round trip, so
/// the directories of one depth are listed at the same time. Levels stay
/// sequential: a task never waits on another, so a bounded pool cannot stall.
const LISTERS: usize = 16;

/// Lists what `dir` holds and would go. Nothing is written; `Candidate::delete`
/// does that. Skipped directories are reported on `log` at debug level.
pub fn plan(log: &dyn Log, dir: &VfsPath, o: &Options) -> Result<Plan, Error> {
    if !dir.is_dir()? {
        return Err(Error::NotADirectory(dir.as_str().to_owned()));
    }

    let mut plan = Plan::default();
    let mut level = vec![dir.clone()];

    while !level.is_empty() {
        let mut listed = Vec::with_capacity(level.len());
        run_each(LISTERS, level, |d| scan(&d, o), |l| listed.push(l));
        let listed = in_listing_order(listed)?;

        level = Vec::new();
        for l in listed {
            level.extend(l.subdirs);
            plan.candidates.extend(l.candidates);

            let Some(sk) = l.skipped else { continue };
            log.debug(&format!(
                "skip {}: no {CONVERTED_DIR}/",
                relative(dir, &sk.dir)
            ));
            // A directory without raws is just a container (the year folder,
            // a JPEG-only shoot); only one holding raws is worth a recap line.
            if sk.raws > 0 {
                plan.skipped.push(sk);
            }
        }
    }

    for c in &mut plan.candidates {
        c.kept = kept(&relative(dir, &c.raw.path), o);
    }

    plan.candidates
        .sort_by(|a, b| a.raw.path.as_str().cmp(b.raw.path.as_str()));
    plan.skipped
        .sort_by(|a, b| a.dir.as_str().cmp(b.dir.as_str()));

    Ok(plan)
}

/// The pool hands listings back as they complete; the debug lines should
/// follow the directory order, and the first error stops the plan.
fn in_listing_order(listed: Vec<Result<Listed, VfsError>>) -> Result<Vec<Listed>, VfsError> {
    let mut ok = Vec::with_capacity(listed.len());
    for l in listed {
        ok.push(l?);
    }
    ok.sort_by(|a, b| a.dir.as_str().cmp(b.dir.as_str()));
    Ok(ok)
}

struct Listed {
    dir: VfsPath,
    candidates: Vec<Candidate>,
    subdirs: Vec<VfsPath>,
    skipped: Option<Skipped>,
}

struct Entry {
    path: VfsPath,
    meta: VfsMetadata,
}

impl Entry {
    fn is_file(&self) -> bool {
        self.meta.file_type == VfsFileType::File
    }

    fn is_dir(&self) -> bool {
        self.meta.file_type == VfsFileType::Directory
    }
}

fn entries(dir: &VfsPath) -> Result<Vec<Entry>, VfsError> {
    let mut out = Vec::new();
    for path in dir.read_dir()? {
        let meta = path.metadata()?;
        out.push(Entry { path, meta });
    }
    Ok(out)
}

/// Lists one directory once and takes everything from that listing: the raws,
/// their sidecars, whether converted/ exists and, when recursive, where to descend.
fn scan(dir: &VfsPath, o: &Options) -> Result<Listed, VfsError> {
    let entries = entries(dir)?;

    let mut by_name: HashMap<String, &Entry> = HashMap::new();
    for e in &entries {
        by_name.insert(e.path.filename(), e);
    }

    let mut got = Listed {
        dir: dir.clone(),
        candidates: Vec::new(),
        subdirs: Vec::new(),
        skipped: None,
    };

    let converted = converted_names(by_name.get(CONVERTED_DIR).copied());
    match &converted {
        Some(c) => {
            for e in &entries {
                if let Some(c) = candidate(e, &by_name, &c.names, o) {
                    got.candidates.push(c);
                }
            }
        }
        None => got.skipped = Some(waiting(dir, &entries, o)),
    }

    if o.bordered {
        let mut dirs = match converted {
            Some(c) => c.bordered,
            None => Vec::new(),
        };
        for e in &entries {
            if e.is_dir() && is_bordered_dir(&e.path.filename()) {
                dirs.push(e.path.clone());
            }
        }
        got.candidates.extend(bordered_in(&dirs)?);
    }

    if o.recursive {
        for e in &entries {
            if e.is_dir() && !skipped_dir(&e.path.filename()) {
                got.subdirs.push(e.path.clone());
            }
        }
    }

    Ok(got)
}

/// Hidden directories, RawTherapee's output and wba's own, none of which
/// holds raws.
fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') || name == CONVERTED_DIR || is_bordered_dir(name)
}

/// Where wba border writes, and where the script that preceded it did.
fn is_bordered_dir(name: &str) -> bool {
    name == DEFAULT_OUT || name == LEGACY_OUT
}

/// The Instagram copies in the bordered folders: images only, so that a note
/// left there is never deleted.
fn bordered_in(dirs: &[VfsPath]) -> Result<Vec<Candidate>, VfsError> {
    let mut copies = Vec::new();
    for d in dirs {
        for e in entries(d)? {
            if e.is_file() && is_image(&e.path.filename()) {
                copies.push(Candidate {
                    raw: File {
                        path: e.path,
                        size: e.meta.len,
                    },
                    sidecar: None,
                    kept: None,
                    bordered: true,
                });
            }
        }
    }
    Ok(copies)
}

/// The raws clean will not touch, so the recap can say what sits there unsorted.
fn waiting(dir: &VfsPath, entries: &[Entry], o: &Options) -> Skipped {
    let mut skipped = Skipped {
        dir: dir.clone(),
        raws: 0,
        size: 0,
    };
    for e in entries {
        if e.is_file() && o.pattern.matches(&e.path.filename()) {
            skipped.raws += 1;
            skipped.size += e.meta.len;
        }
    }
    skipped
}

fn candidate(
    e: &Entry,
    by_name: &HashMap<String, &Entry>,
    developed: &[String],
    o: &Options,
) -> Option<Candidate> {
    let name = e.path.filename();
    if !e.is_file() || !o.pattern.matches(&name) {
        return None;
    }

    let stem = stem(&name).to_ascii_lowercase();
    if developed.iter().any(|d| d.starts_with(&stem)) {
        return None;
    }

    let mut sidecar = None;
    if let Some(s) = by_name.get(&format!("{name}{SIDECAR_EXT}"))
        && s.is_file()
    {
        sidecar = Some(File {
            path: s.path.clone(),
            size: s.meta.len,
        });
    }

    Some(Candidate {
        raw: File {
            path: e.path.clone(),
            size: e.meta.len,
        },
        sidecar,
        kept: None,
        bordered: false,
    })
}

struct Converted {
    /// The lowercased names of its regular files.
    names: Vec<String>,
    /// The bordered folders inside it: border enters converted/, so its
    /// output lands there as often as beside it.
    bordered: Vec<VfsPath>,
}

/// What converted/ holds, given its entry from the parent listing; `None`
/// means "not a photo directory".
fn converted_names(converted: Option<&Entry>) -> Option<Converted> {
    let converted = converted?;
    if !converted.is_dir() {
        return None;
    }
    let Ok(entries) = entries(&converted.path) else {
        return None;
    };

    let mut got = Converted {
        names: Vec::new(),
        bordered: Vec::new(),
    };
    for e in entries {
        if e.is_file() {
            got.names.push(e.path.filename().to_ascii_lowercase());
        } else if e.is_dir() && is_bordered_dir(&e.path.filename()) {
            got.bordered.push(e.path);
        }
    }
    Some(got)
}

/// Why a candidate stays: an --exclude glob it matches, or no --only glob
/// matching it when --only is given.
fn kept(rel: &str, o: &Options) -> Option<Kept> {
    if let Some(g) = glob::matching(rel, &o.exclude) {
        return Some(Kept::Excluded(g.clone()));
    }
    if !o.only.is_empty() && glob::matching(rel, &o.only).is_none() {
        return Some(Kept::NotInOnly(o.only.clone()));
    }
    None
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;

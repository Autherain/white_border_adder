use std::collections::HashSet;

use crate::log::Log;
use crate::path::relative;
use vfs::error::VfsErrorKind;
use vfs::{VfsError, VfsPath};

use super::{Job, LEGACY_OUT, Options, Out};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: not a directory")]
    NotADirectory(String),
    #[error("read {0}: {1}")]
    Read(String, VfsError),
    #[error(transparent)]
    Fs(#[from] VfsError),
}

/// Lists the images under `dir` and where their bordered copy goes. Nothing
/// is written; `Job::run` does that. Skipped files are reported on `log` at
/// debug level.
pub fn plan(log: &dyn Log, dir: &VfsPath, o: &Options) -> Result<Vec<Job>, Error> {
    if !dir.is_dir()? {
        return Err(Error::NotADirectory(dir.as_str().to_owned()));
    }

    let mut p = Planner {
        log,
        dir,
        o,
        jobs: Vec::new(),
    };
    p.scan(dir)?;
    p.jobs
        .sort_by(|a, b| a.input.as_str().cmp(b.input.as_str()));
    Ok(p.jobs)
}

struct Planner<'a> {
    log: &'a dyn Log,
    dir: &'a VfsPath,
    o: &'a Options,
    jobs: Vec<Job>,
}

impl Planner<'_> {
    /// Lists `dir` once: its images become jobs and, when recursive, its
    /// subdirectories are scanned in turn. One listing per directory and
    /// one per output directory is what keeps a network mount bearable.
    fn scan(&mut self, dir: &VfsPath) -> Result<(), Error> {
        let entries: Vec<VfsPath> = match dir.read_dir() {
            Ok(entries) => entries.collect(),
            Err(e) => return Err(Error::Read(dir.as_str().to_owned(), e)),
        };
        let existing = self.existing(dir)?;

        for entry in entries {
            if entry.is_dir()? {
                if self.o.recursive && !self.skip_dir(&entry.filename()) {
                    self.scan(&entry)?;
                }
            } else if entry.is_file()? {
                self.add(dir, entry, &existing);
            }
        }
        Ok(())
    }

    /// What the output directory already holds, so that checking each image
    /// costs a set lookup instead of a stat. A missing output directory holds
    /// nothing; `force` makes the question moot.
    fn existing(&self, photo_dir: &VfsPath) -> Result<HashSet<String>, Error> {
        if self.o.force {
            return Ok(HashSet::new());
        }

        let out = self.out_dir(photo_dir)?;
        let entries = match out.read_dir() {
            Ok(entries) => entries,
            Err(e) if matches!(e.kind(), VfsErrorKind::FileNotFound) => return Ok(HashSet::new()),
            Err(e) => return Err(Error::Read(out.as_str().to_owned(), e)),
        };

        let mut names = HashSet::new();
        for e in entries {
            names.insert(e.filename());
        }
        Ok(names)
    }

    /// Keeps the walk out of hidden directories and of wba's own outputs, so
    /// a second run never borders a bordered copy. RawTherapee's converted/
    /// is entered on purpose: that is where the exports to border live.
    fn skip_dir(&self, name: &str) -> bool {
        name.starts_with('.') || name == LEGACY_OUT || name == self.out_base()
    }

    fn out_base(&self) -> String {
        match &self.o.out {
            Out::Beside(rel) => rel.rsplit('/').next().unwrap_or(rel).to_owned(),
            Out::At(path) => path.filename(),
        }
    }

    fn out_dir(&self, photo_dir: &VfsPath) -> Result<VfsPath, VfsError> {
        match &self.o.out {
            Out::Beside(rel) => photo_dir.join(rel),
            Out::At(path) => Ok(path.clone()),
        }
    }

    fn add(&mut self, dir: &VfsPath, input: VfsPath, existing: &HashSet<String>) {
        let name = input.filename();
        if !is_image(&name) {
            return;
        }

        let out_name = format!("{}{name}", self.o.prefix);
        if existing.contains(&out_name) {
            self.log.debug(&format!(
                "skip {}: output exists",
                relative(self.dir, &input)
            ));
            return;
        }

        let Ok(out_dir) = self.out_dir(dir) else {
            return;
        };
        let Ok(output) = out_dir.join(&out_name) else {
            return;
        };
        self.jobs.push(Job { input, output });
    }
}

pub(crate) fn is_image(name: &str) -> bool {
    matches!(crate::path::ext(name).as_str(), ".jpg" | ".jpeg" | ".png")
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;

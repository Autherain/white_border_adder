use crate::clean::{Candidate, Options, Skipped};
use crate::glob::Glob;
use clap::Args;

use super::show::{Item, Stage, Verb, Warn};
use super::{App, failures, human, show};

/// Delete raws that RawTherapee never developed
#[derive(Args)]
#[command(
    long_about = "Delete the raws of <dir> that RawTherapee never developed.

What gets deleted, step by step:

  1. <dir> must contain a converted/ subdirectory, the folder RawTherapee
     exports into. Without it nothing happens there: no converted/ means no
     sorting was done. With --recursive every subdirectory is checked the
     same way; those without converted/ are skipped.
  2. A raw is a file at the root of that directory matching --pattern
     (*.rw2 by default, any case). Raws in subdirectories or inside
     converted/ are never touched.
  3. A raw is developed when any file in converted/ starts with the raw's
     name without extension: IMG_1264.RW2 is developed by IMG_1264.jpg,
     IMG_1264_bw.tif or IMG_1264-2.png. The extension does not matter.
  4. Every raw that is not developed is deleted, with its RawTherapee
     sidecar (IMG_1264.RW2.pp3) when there is one. The sidecar of a raw
     that stays, stays.

--bordered adds the Instagram copies written by \"wba border\" to the list: the
images in bordered/ and converted/bordered/ of every directory (and in the
old bordered_images/), which are only there to be posted. They go whether or
not the photo they were made from is still around, and the folder goes once
empty. --exclude and --only apply to them like to raws, so --only '*suisse*'
cleans the copies of one shoot. converted/ itself is never touched.

Nothing is deleted without --apply: the default run prints one line per
file, a banner per directory with its total, and the grand total.

A line ending in \"####### has a .pp3 but nothing in converted/\" is a raw
you opened in RawTherapee (the .pp3 holds your edits) but never exported.
Look at it before --apply, or keep it with --exclude.

--exclude takes a glob and keeps every raw whose name without extension
matches it (--exclude IMG_1264, --exclude 'IMG_12*'), or whose path below
<dir> has a directory matching it (--exclude '*marie*' keeps a whole
shoot). --only is the mirror: when given, only the raws matching one of
its globs go, everything else is kept (--only '*suisse*' cleans one
shoot out of a year). --exclude wins when both match. Repeat either flag
for several patterns; case does not matter. The recap at the end lists
what each kept, with sizes."
)]
pub(crate) struct CleanArgs {
    dir: String,
    /// Glob on the file name that marks a raw
    #[arg(long, default_value = "*.rw2")]
    pattern: Glob,
    /// Glob on a raw's name without extension or on a directory of its path, to keep; repeatable
    #[arg(long)]
    exclude: Vec<Glob>,
    /// Same kind of glob; when given, only the raws it matches go, the rest is kept; repeatable
    #[arg(long)]
    only: Vec<Glob>,
    /// Also delete the white-bordered Instagram copies (bordered/, converted/bordered/, bordered_images/)
    #[arg(long)]
    bordered: bool,
}

impl App {
    pub(crate) fn clean(&self, args: CleanArgs) -> anyhow::Result<()> {
        let dir = self.locate(&args.dir)?;
        let options = Options {
            pattern: args.pattern,
            exclude: args.exclude,
            only: args.only,
            recursive: self.recursive,
            bordered: args.bordered,
        };

        let plan = crate::clean::plan(&*self.log, &dir, &options)?;
        let skips = skipped_items(&dir, &plan.skipped);
        if plan.candidates.is_empty() && skips.is_empty() {
            self.log.info(&self.palette.faint("nothing to do"));
            return Ok(());
        }

        let items = clean_items(&dir, &plan.candidates);
        if !self.show(Verb::Delete, &items, &skips) {
            return Ok(());
        }

        let failed = self.delete_all(&dir, not_kept(plan.candidates));
        for line in self.recap(Verb::Delete, &items, &skips, Stage::Finished) {
            self.log.info(&line);
        }
        failures(failed)
    }

    /// Removes the candidates one by one; a failure is reported and counted,
    /// the rest goes on.
    fn delete_all(&self, dir: &vfs::VfsPath, candidates: Vec<Candidate>) -> usize {
        let mut failed = 0;
        for candidate in candidates {
            match candidate.delete() {
                Ok(removed) => {
                    for path in removed {
                        self.log.info(&format!(
                            "{} {}",
                            self.palette.gone("deleted"),
                            Self::rel(dir, &path)
                        ));
                    }
                }
                Err(failure) => {
                    failed += 1;
                    let path = Self::rel(dir, &failure.path);
                    self.log.error(&format!("{path}: {}", failure.source));
                }
            }
        }
        failed
    }
}

fn not_kept(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut to_delete = Vec::new();
    for candidate in candidates {
        if candidate.kept.is_none() {
            to_delete.push(candidate);
        }
    }
    to_delete
}

fn clean_items(dir: &vfs::VfsPath, candidates: &[Candidate]) -> Vec<Item> {
    let mut items = Vec::new();
    for candidate in candidates {
        items.push(clean_item(dir, candidate));
    }
    items
}

fn skipped_items(dir: &vfs::VfsPath, skipped: &[Skipped]) -> Vec<show::Skipped> {
    let mut items = Vec::new();
    for one in skipped {
        items.push(skipped_item(dir, one));
    }
    items
}

/// The printed form of a candidate: its files and its folder, relative to
/// the argument.
fn clean_item(dir: &vfs::VfsPath, candidate: &Candidate) -> Item {
    let mut files = Vec::new();
    for file in candidate.files() {
        files.push(crate::path::relative(dir, &file.path));
    }
    // A sidecar without an export means time was spent on this raw in
    // RawTherapee; worth a look before it goes.
    let mut warn = None;
    if candidate.sidecar.is_some() {
        warn = Some(Warn::EditedNeverExported);
    }
    let kept = match &candidate.kept {
        Some(why) => Some(why.to_string()),
        None => None,
    };
    Item {
        dir: crate::path::relative(dir, &candidate.file.path.parent()),
        files,
        size: candidate.size(),
        warn,
        kept,
    }
}

fn skipped_item(dir: &vfs::VfsPath, skipped: &Skipped) -> show::Skipped {
    let mut raws = "raws";
    if skipped.raws == 1 {
        raws = "raw";
    }
    show::Skipped {
        dir: crate::path::relative(dir, &skipped.dir),
        detail: format!(
            "{} {raws}, {}, no converted/: not sorted yet",
            skipped.raws,
            human(skipped.size)
        ),
    }
}

#[cfg(test)]
#[path = "clean_test.rs"]
mod tests;

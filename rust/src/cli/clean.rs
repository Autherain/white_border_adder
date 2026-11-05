use crate::clean::{Candidate, Options, Skipped};
use crate::glob::Glob;
use crate::log::Log;
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
}

impl<L: Log> App<L> {
    pub(crate) fn clean(&self, args: CleanArgs) -> anyhow::Result<()> {
        let dir = self.locate(&args.dir)?;
        let o = Options {
            pattern: args.pattern,
            exclude: args.exclude,
            only: args.only,
            recursive: self.recursive,
        };

        let plan = crate::clean::plan(&self.log, &dir, &o)?;
        let skips: Vec<show::Skipped> = plan
            .skipped
            .iter()
            .map(|sk| skipped_item(&dir, sk))
            .collect();

        if plan.candidates.is_empty() && skips.is_empty() {
            self.log.info(&self.palette.faint("nothing to do"));
            return Ok(());
        }

        let items: Vec<Item> = plan
            .candidates
            .iter()
            .map(|c| clean_item(&dir, c))
            .collect();
        if !self.show(Verb::Delete, &items, &skips) {
            return Ok(());
        }

        let failed = self.delete_all(
            &dir,
            plan.candidates.into_iter().filter(|c| c.kept.is_none()),
        );
        for line in self.recap(Verb::Delete, &items, &skips, Stage::Finished) {
            self.log.info(&line);
        }
        failures(failed)
    }

    /// Removes the candidates one by one; a failure is reported and counted,
    /// the rest goes on.
    fn delete_all(&self, dir: &vfs::VfsPath, candidates: impl Iterator<Item = Candidate>) -> usize {
        let mut failed = 0;
        for c in candidates {
            match c.delete() {
                Ok(removed) => {
                    for p in removed {
                        self.log.info(&format!(
                            "{} {}",
                            self.palette.gone("deleted"),
                            Self::rel(dir, &p)
                        ));
                    }
                }
                Err(e) => {
                    failed += 1;
                    self.log
                        .error(&format!("{}: {}", Self::rel(dir, &e.path), e.source));
                }
            }
        }
        failed
    }
}

fn clean_item(dir: &vfs::VfsPath, c: &Candidate) -> Item {
    Item {
        dir: crate::path::relative(dir, &c.raw.path.parent()),
        files: c
            .files()
            .map(|f| crate::path::relative(dir, &f.path))
            .collect(),
        size: c.size(),
        // A sidecar without an export means time was spent on this raw in
        // RawTherapee; worth a look before it goes.
        warn: if c.sidecar.is_some() {
            Some(Warn::EditedNeverExported)
        } else {
            None
        },
        kept: c.kept.as_ref().map(ToString::to_string),
    }
}

fn skipped_item(dir: &vfs::VfsPath, sk: &Skipped) -> show::Skipped {
    let raws = if sk.raws == 1 { "raw" } else { "raws" };
    show::Skipped {
        dir: crate::path::relative(dir, &sk.dir),
        detail: format!(
            "{} {raws}, {}, no converted/: not sorted yet",
            sk.raws,
            human(sk.size)
        ),
    }
}

#[cfg(test)]
#[path = "clean_test.rs"]
mod tests;

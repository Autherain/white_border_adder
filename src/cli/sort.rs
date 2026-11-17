use crate::glob::Glob;
use crate::path::relative;
use crate::sorter::{ExifOrMtime, Move, Options};
use clap::Args;
use vfs::VfsPath;

use super::show::{Item, Stage, Verb};
use super::{App, failures};

/// File photos under <year>/<day folder> by the date they were shot
#[derive(Args)]
#[command(
    long_about = "File every photo under <dir>/<year>/<day folder>, by the date it was shot.

Run it on the directory that holds your 2025/ and 2026/ folders, or from inside
one of them: a <dir> named like a year is searched alone and the photos that
belong to another year leave it for that year's folder. Without
[source] it re-files everything below <dir>, which fixes photos moved into
the wrong day folder; give a [source] directory (a dump of unsorted photos,
inside or outside <dir>) to file only what is in it. Use --recursive to go
into subdirectories.

The date is the one the camera wrote inside the file (EXIF), read from the
first 256 kB only. A photo without one, a PNG for instance, falls back to its
modification time.

The day folder is chosen in this order:
  1. the dated folder the photo already sits in, when its date is right:
     the photo stays, nothing is listed;
  2. the folder of that day already under the year (20260620_toulouse), so
     the place name you typed is kept;
  3. a new folder named after the day (20260620).
When several folders exist for a day the photo is kept and listed: only you
know which is right.

Raws, JPEG and PNG move. A raw's .pp3 sidecar and its developed copies in
converted/ (IMG_1264.jpg, IMG_1264-2.png for IMG_1264.RW2, not IMG_12645.jpg)
move with it, into <day folder>/converted/. converted/ and bordered/ are
never scanned as photos, and bordered/ is never moved: it is not original.
Nothing is overwritten, and emptied folders are left in place.

Old Nikon photos (DSC_* and any folder with nikon in its name) are always
left alone, without the flag. --exclude leaves alone every photo whose name
without extension matches the glob, or that has a directory in its path
matching it, and does not even read it: --exclude 'DSC_*' for the old Nikon
numbering, --exclude '*nikon*' for a whole folder. Repeat it for several;
case does not matter. Add --verbose to see what was left out.

Nothing moves without --apply."
)]
pub(crate) struct SortArgs {
    dir: String,
    sources: Vec<String>,
    /// Glob on a photo's name without extension or on a directory of its path, to leave alone; repeatable
    #[arg(long)]
    exclude: Vec<Glob>,
}

impl App {
    pub(crate) fn sort(&self, args: SortArgs) -> anyhow::Result<()> {
        let dir = self.locate(&args.dir)?;
        let mut sources = Vec::new();
        for source in &args.sources {
            sources.push(self.locate(source)?);
        }
        let options = Options {
            recursive: self.recursive,
            exclude: args.exclude,
        };

        let moves = crate::sorter::plan(&*self.log, &ExifOrMtime, &dir, &sources, &options)?;
        if moves.is_empty() {
            self.log.info(&self.palette.faint("nothing to do"));
            return Ok(());
        }

        let items = sort_items(&dir, &moves);
        if !self.show(Verb::Move, &items, &[]) {
            return Ok(());
        }

        let failed = self.move_all(&dir, not_kept(moves));
        for line in self.recap(Verb::Move, &items, &[], Stage::Finished) {
            self.log.info(&line);
        }
        failures(failed)
    }

    fn move_all(&self, dir: &VfsPath, moves: Vec<Move>) -> usize {
        let mut failed = 0;
        for planned in moves {
            match planned.run() {
                Ok(pairs) => {
                    for pair in pairs {
                        let line = format!(
                            "{} {} -> {}",
                            self.palette.done("moved"),
                            Self::rel(dir, &pair.from),
                            Self::rel(dir, &pair.to)
                        );
                        self.log.info(&line);
                    }
                }
                Err(failure) => {
                    failed += 1;
                    let from = Self::rel(dir, &failure.from);
                    self.log.error(&format!("{from}: {}", failure.source));
                }
            }
        }
        failed
    }
}

fn not_kept(moves: Vec<Move>) -> Vec<Move> {
    let mut to_run = Vec::new();
    for planned in moves {
        if planned.kept.is_none() {
            to_run.push(planned);
        }
    }
    to_run
}

fn sort_items(dir: &VfsPath, moves: &[Move]) -> Vec<Item> {
    let mut items = Vec::new();
    for planned in moves {
        items.push(sort_item(dir, planned));
    }
    items
}

/// The printed form of a move: one "from -> to" line per file, grouped
/// under the folder the photo goes to.
fn sort_item(dir: &VfsPath, planned: &Move) -> Item {
    let kept = match &planned.kept {
        Some(why) => Some(why.to_string()),
        None => None,
    };
    let to = match &planned.to {
        Some(to) => to,
        None => return unmoved_item(dir, planned, kept),
    };

    let mut files = Vec::new();
    for pair in planned.pairs() {
        files.push(format!(
            "{} -> {}",
            relative(dir, &pair.from),
            relative(dir, &pair.to)
        ));
    }
    Item {
        dir: relative(dir, &to.parent()),
        files,
        size: 0,
        warn: None,
        kept,
    }
}

/// A kept move without a target touches nothing and is listed by its source.
fn unmoved_item(dir: &VfsPath, planned: &Move, kept: Option<String>) -> Item {
    Item {
        dir: relative(dir, &planned.from.parent()),
        files: vec![relative(dir, &planned.from)],
        size: 0,
        warn: None,
        kept,
    }
}

#[cfg(test)]
#[path = "sort_test.rs"]
mod tests;

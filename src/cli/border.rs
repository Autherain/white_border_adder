use crate::border::{Job, Options, Out, Ratios};
use crate::path::relative;
use crate::runeach::run_each;
use clap::Args;

use super::show::{Item, Stage, Verb};
use super::{App, failures};

/// Instagram's square post; the ratios are what looked right on the owner's photos.
const DEFAULT_SIZE: u32 = 1080;

fn cpus() -> usize {
    match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    }
}

/// Write a white-bordered copy of every JPEG and PNG in <dir>
#[derive(Args)]
#[command(
    long_about = "Write a white-bordered copy of every JPEG and PNG at the root of <dir>.

Each photo is scaled to fit inside a white 1080x1080 canvas (Instagram's
square) minus the border ratios, centred, and written as
<dir>/bordered/bordered_<name>. Landscape and portrait photos get their
own ratios. The originals are never modified.

An output file that already exists is skipped unless --force. Nothing is
written without --apply."
)]
pub(crate) struct BorderArgs {
    dir: String,
    /// Output width
    #[arg(long, default_value_t = DEFAULT_SIZE)]
    width: u32,
    /// Output height
    #[arg(long, default_value_t = DEFAULT_SIZE)]
    height: u32,
    /// Vertical border ratio for landscape photos
    #[arg(long, default_value_t = 0.05)]
    landscape_vert: f64,
    /// Horizontal border ratio for landscape photos
    #[arg(long, default_value_t = 0.03)]
    landscape_horiz: f64,
    /// Vertical border ratio for portrait photos
    #[arg(long, default_value_t = 0.005)]
    portrait_vert: f64,
    /// Horizontal border ratio for portrait photos
    #[arg(long, default_value_t = 0.18)]
    portrait_horiz: f64,
    /// JPEG output quality, 1-100
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u8).range(1..=100))]
    jpeg_quality: u8,
    /// Prefix of the output file name
    #[arg(long, default_value = "bordered_")]
    prefix: String,
    /// Output directory (default <dir>/bordered, per photo directory with --recursive)
    #[arg(long)]
    out: Option<String>,
    /// Overwrite an output file that already exists
    #[arg(long)]
    force: bool,
    /// Number of photos processed at once
    #[arg(long, default_value_t = cpus())]
    workers: usize,
}

impl App {
    pub(crate) fn border(&self, args: BorderArgs) -> anyhow::Result<()> {
        let dir = self.locate(&args.dir)?;
        let workers = args.workers;
        let options = self.border_options(args)?;

        let jobs = crate::border::plan(&*self.log, &dir, &options)?;
        if jobs.is_empty() {
            self.log.info(&self.palette.faint("nothing to do"));
            return Ok(());
        }

        let items = border_items(&dir, &jobs);
        if !self.show(Verb::Write, &items, &[]) {
            return Ok(());
        }

        let failed = self.write_all(&dir, jobs, &options, workers);
        for line in self.recap(Verb::Write, &items, &[], Stage::Finished) {
            self.log.info(&line);
        }
        failures(failed)
    }

    fn border_options(&self, args: BorderArgs) -> anyhow::Result<Options> {
        let out = self.out_of(args.out)?;
        Ok(Options {
            width: args.width,
            height: args.height,
            landscape: Ratios {
                vert: args.landscape_vert,
                horiz: args.landscape_horiz,
            },
            portrait: Ratios {
                vert: args.portrait_vert,
                horiz: args.portrait_horiz,
            },
            jpeg_quality: args.jpeg_quality,
            prefix: args.prefix,
            out,
            recursive: self.recursive,
            force: args.force,
        })
    }

    /// An absolute --out is one directory for every copy; a relative one is
    /// created beside each photo directory.
    fn out_of(&self, flag: Option<String>) -> anyhow::Result<Out> {
        let path = match flag {
            Some(p) => p,
            None => return Ok(Out::default()),
        };
        if std::path::Path::new(&path).is_absolute() {
            return Ok(Out::At(self.locate(&path)?));
        }
        Ok(Out::Beside(path))
    }

    /// Runs the jobs `workers` at a time, decoding being the slow part,
    /// and prints each result as it comes. Returns how many failed.
    fn write_all(
        &self,
        dir: &vfs::VfsPath,
        jobs: Vec<Job>,
        options: &Options,
        workers: usize,
    ) -> usize {
        let mut failed = 0;
        run_each(
            workers,
            jobs,
            |job| job.run(options),
            |result| match result {
                Ok(output) => self.log.info(&format!(
                    "{} {}",
                    self.palette.done("wrote"),
                    Self::rel(dir, &output)
                )),
                Err(failure) => {
                    failed += 1;
                    let input = Self::rel(dir, &failure.input);
                    self.log.error(&format!("{input}: {}", failure.source));
                }
            },
        );
        failed
    }
}

fn border_items(dir: &vfs::VfsPath, jobs: &[Job]) -> Vec<Item> {
    let mut items = Vec::new();
    for job in jobs {
        items.push(border_item(dir, job));
    }
    items
}

fn border_item(dir: &vfs::VfsPath, job: &Job) -> Item {
    Item {
        dir: relative(dir, &job.input.parent()),
        files: vec![relative(dir, &job.output)],
        size: 0,
        warn: None,
        kept: None,
    }
}

#[cfg(test)]
#[path = "border_test.rs"]
mod tests;

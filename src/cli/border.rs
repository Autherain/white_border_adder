use crate::border::{Job, Options, Out, Ratios};
use crate::log::Log;
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

impl<L: Log> App<L> {
    pub(crate) fn border(&self, args: BorderArgs) -> anyhow::Result<()> {
        let dir = self.locate(&args.dir)?;
        let out = match args.out {
            None => Out::default(),
            Some(p) if std::path::Path::new(&p).is_absolute() => Out::At(self.locate(&p)?),
            Some(p) => Out::Beside(p),
        };
        let o = Options {
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
        };

        let jobs = crate::border::plan(&self.log, &dir, &o)?;
        if jobs.is_empty() {
            self.log.info(&self.palette.faint("nothing to do"));
            return Ok(());
        }

        let items: Vec<Item> = jobs
            .iter()
            .map(|j| Item {
                dir: crate::path::relative(&dir, &j.input.parent()),
                files: vec![crate::path::relative(&dir, &j.output)],
                ..Item::default()
            })
            .collect();
        if !self.show(Verb::Write, &items, &[]) {
            return Ok(());
        }

        let failed = self.write_all(&dir, jobs, &o, args.workers);
        for line in self.recap(Verb::Write, &items, &[], Stage::Finished) {
            self.log.info(&line);
        }
        failures(failed)
    }

    fn write_all(&self, dir: &vfs::VfsPath, jobs: Vec<Job>, o: &Options, workers: usize) -> usize {
        let mut failed = 0;
        run_each(
            workers,
            jobs,
            |j| j.run(o),
            |r| match r {
                Ok(output) => self.log.info(&format!(
                    "{} {}",
                    self.palette.done("wrote"),
                    Self::rel(dir, &output)
                )),
                Err(e) => {
                    failed += 1;
                    self.log
                        .error(&format!("{}: {}", Self::rel(dir, &e.input), e.source));
                }
            },
        );
        failed
    }
}

#[cfg(test)]
#[path = "border_test.rs"]
mod tests;

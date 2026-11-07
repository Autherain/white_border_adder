//! Flags, output lines and exit codes. Every verb plans, shows the plan or,
//! with --apply, runs it; the modules below do the work.

mod border;
mod clean;
mod show;
mod sort;
mod style;

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand};
use vfs::VfsPath;

use crate::log::{Lines, Log};
use style::Palette;

/// Set at build time with `WBA_VERSION=...`, as `git describe` names it.
const VERSION: &str = match option_env!("WBA_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Parser)]
#[command(name = "wba", version = VERSION, disable_version_flag = true)]
#[command(about = "White borders for Instagram and cleanup of undeveloped raws")]
#[command(long_about = "wba works on a directory of photos, one verb at a time.

Every verb is a dry run by default: it prints one \"would ...\" line per
file and a total, and changes nothing. Add --apply to do it. Read the
dry run first, especially before \"clean\", which deletes files.

  wba clean <dir>             list the raws that would be deleted
  wba clean <dir> --apply     delete them
  wba clean --help            what \"developed\" and \"candidate\" mean
  wba sort <dir> --recursive  list the photos that would move to <year>/<day folder>")]
struct Cli {
    #[command(flatten)]
    shared: Shared,
    #[command(subcommand)]
    verb: Verb,
}

#[derive(Args)]
struct Shared {
    /// Do it; without this flag nothing is written or deleted
    #[arg(long, global = true)]
    apply: bool,
    /// Walk the tree below <dir>
    #[arg(long, global = true)]
    recursive: bool,
    /// Also print skipped items and why
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Verb {
    Border(border::BorderArgs),
    Clean(clean::CleanArgs),
    Sort(sort::SortArgs),
    /// Print the version
    Version,
    /// Print the completion script for a shell
    Completion {
        shell: clap_complete::Shell,
    },
}

/// Turns the argument the user typed into a path on the filesystem the app
/// works on: the OS one in `main`, a tree in memory in the tests.
pub type Locate = Box<dyn Fn(&str) -> anyhow::Result<VfsPath>>;

struct App {
    // A trait object rather than a type parameter: the logger is a behaviour,
    // not a data type, and a generic `App<L>` would spread `<L>` over every
    // impl and test signature. `&*self.log` hands the `&dyn Log` out of the box.
    log: Box<dyn Log>,
    locate: Locate,
    palette: Palette,
    apply: bool,
    recursive: bool,
}

/// Parses `args` (the program name first) and runs the verb. `colour` says
/// whether `out` is a terminal.
pub fn run(
    locate: Locate,
    args: impl IntoIterator<Item = OsString>,
    mut out: impl Write + Send + 'static,
    mut err: impl Write + Send + 'static,
    colour: bool,
) -> ExitCode {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let text = e.render();
            if e.use_stderr() {
                let _ = write!(err, "{text}");
            } else {
                let _ = write!(out, "{text}");
            }
            let code = match u8::try_from(e.exit_code()) {
                Ok(c) => c,
                Err(_) => 2,
            };
            return ExitCode::from(code);
        }
    };

    match cli.verb {
        Verb::Version => {
            let _ = writeln!(out, "{VERSION}");
            return ExitCode::SUCCESS;
        }
        Verb::Completion { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "wba", &mut out);
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let app = App {
        log: Box::new(Lines::new(out, err, cli.shared.verbose)),
        locate,
        palette: Palette::new(colour),
        apply: cli.shared.apply,
        recursive: cli.shared.recursive,
    };

    let result = match cli.verb {
        Verb::Border(args) => app.border(args),
        Verb::Clean(args) => app.clean(args),
        Verb::Sort(args) => app.sort(args),
        Verb::Version | Verb::Completion { .. } => Ok(()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            app.log.error(&format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}

impl App {
    fn locate(&self, arg: &str) -> anyhow::Result<VfsPath> {
        (self.locate)(arg)
    }

    /// Prints paths the way the user typed the argument, not as the fs joined them.
    fn rel(dir: &VfsPath, path: &VfsPath) -> String {
        crate::path::relative(dir, path)
    }
}

fn files(n: usize) -> String {
    if n == 1 {
        return "1 file".to_owned();
    }
    format!("{n} files")
}

/// The verb's error once it ran: how many items failed, or nothing.
fn failures(n: usize) -> anyhow::Result<()> {
    if n == 0 {
        return Ok(());
    }
    Err(anyhow::anyhow!("{} failed", files(n)))
}

/// Rounds to the unit a photographer thinks in: raws are tens of MB.
#[allow(clippy::cast_precision_loss)]
fn human(bytes: u64) -> String {
    const KB: u64 = 1000;
    const MB: u64 = 1000 * KB;
    const GB: u64 = 1000 * MB;

    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= 10 * MB => format!("{} MB", (b + MB / 2) / MB),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{} kB", (b + KB / 2) / KB),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

//! Builds the one filesystem the app works on and hands it to the cli.

mod border;
mod clean;
mod cli;
mod glob;
mod log;
mod path;
mod runeach;
mod sorter;
#[cfg(test)]
mod testfs;

use std::io::IsTerminal;
use std::path::{Component, Path};
use std::process::ExitCode;

use vfs::{PhysicalFS, VfsPath};

fn main() -> ExitCode {
    // The only place that touches the OS filesystem: one PhysicalFS rooted
    // at the top, so a --out or a [source] anywhere on the disk is a path on
    // the same filesystem and a move stays a rename.
    let root = VfsPath::new(PhysicalFS::new(top()));
    let locate: cli::Locate = Box::new(move |arg| locate(&root, arg));

    let colour = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    cli::run(
        locate,
        std::env::args_os(),
        std::io::stdout(),
        std::io::stderr(),
        colour,
    )
}

/// `/` on Unix, the drive of the current directory on Windows.
fn top() -> std::path::PathBuf {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => Path::new("/").to_path_buf(),
    };
    let mut top = std::path::PathBuf::new();
    for c in cwd.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => top.push(c),
            _ => break,
        }
    }
    top
}

fn locate(root: &VfsPath, arg: &str) -> anyhow::Result<VfsPath> {
    let abs = std::path::absolute(arg)?;
    let mut rel = Vec::new();
    for c in abs.components() {
        let Component::Normal(name) = c else { continue };
        let Some(name) = name.to_str() else {
            anyhow::bail!("{arg}: not valid UTF-8");
        };
        rel.push(name);
    }
    Ok(root.join(rel.join("/"))?)
}

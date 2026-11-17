use vfs::{VfsError, VfsPath};

use super::scan::{Scanned, scan};
use super::{CONVERTED_DIR, Kept, Options, Plan, Skipped};
use crate::glob;
use crate::log::Log;
use crate::path::relative;
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

/// Walks `dir` and lists what clean would delete. Nothing is written;
/// `Candidate::delete` does that. Skipped directories are reported on `log`
/// at debug level.
pub fn plan(log: &dyn Log, dir: &VfsPath, options: &Options) -> Result<Plan, Error> {
    if !dir.is_dir()? {
        return Err(Error::NotADirectory(dir.as_str().to_owned()));
    }

    let mut plan = Plan::default();
    let mut level = vec![dir.clone()];
    while !level.is_empty() {
        let scans = scan_level(level, options)?;

        level = Vec::new();
        for scanned in scans {
            level.extend(scanned.subdirs);
            plan.candidates.extend(scanned.candidates);
            if let Some(skipped) = scanned.skipped {
                note_skipped(log, dir, skipped, &mut plan);
            }
        }
    }

    mark_kept(&mut plan, dir, options);
    sort_by_path(&mut plan);
    Ok(plan)
}

/// Scans the directories of one depth. The first error stops the plan.
fn scan_level(level: Vec<VfsPath>, options: &Options) -> Result<Vec<Scanned>, VfsError> {
    let mut results = Vec::with_capacity(level.len());
    run_each(
        LISTERS,
        level,
        |dir| scan(&dir, options),
        |result| results.push(result),
    );

    let mut scans = Vec::with_capacity(results.len());
    for result in results {
        scans.push(result?);
    }
    // The pool hands scans back as they complete; the debug lines should
    // follow the directory order.
    scans.sort_by(|a, b| a.dir.as_str().cmp(b.dir.as_str()));
    Ok(scans)
}

fn note_skipped(log: &dyn Log, root: &VfsPath, skipped: Skipped, plan: &mut Plan) {
    log.debug(&format!(
        "skip {}: no {CONVERTED_DIR}/",
        relative(root, &skipped.dir)
    ));
    // A directory without raws is just a container (the year folder, a
    // JPEG-only shoot); only one holding raws is worth a recap line.
    if skipped.raws > 0 {
        plan.skipped.push(skipped);
    }
}

/// The globs are written against the path below the argument, which only
/// `plan` knows: `scan` sees one directory at a time.
fn mark_kept(plan: &mut Plan, root: &VfsPath, options: &Options) {
    for candidate in &mut plan.candidates {
        candidate.kept = why_kept(&relative(root, &candidate.file.path), options);
    }
}

fn sort_by_path(plan: &mut Plan) {
    plan.candidates
        .sort_by(|a, b| a.file.path.as_str().cmp(b.file.path.as_str()));
    plan.skipped
        .sort_by(|a, b| a.dir.as_str().cmp(b.dir.as_str()));
}

/// Why a candidate stays: an --exclude glob it matches, or no --only glob
/// matching it when --only is given.
fn why_kept(rel: &str, options: &Options) -> Option<Kept> {
    if let Some(glob) = glob::matching(rel, &options.exclude) {
        return Some(Kept::Excluded(glob));
    }
    if options.only.is_empty() {
        return None;
    }
    if glob::matching(rel, &options.only).is_none() {
        return Some(Kept::NotInOnly(options.only.clone()));
    }
    None
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;

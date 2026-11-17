use std::fmt;

use crate::log::BANNER;

use super::App;
use super::recap::{by_directory, summary};

/// What a verb does to a file. It words every line: "would delete",
/// "deleted", "Re-run with --apply to delete them".
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Delete,
    Write,
    Move,
}

impl Verb {
    pub(super) fn past(self) -> &'static str {
        match self {
            Self::Delete => "deleted",
            Self::Write => "wrote",
            Self::Move => "moved",
        }
    }
}

impl fmt::Display for Verb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Delete => "delete",
            Self::Write => "write",
            Self::Move => "move",
        })
    }
}

/// A file that deserves a second look before --apply.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Warn {
    /// Time was spent on this raw in RawTherapee, and it was never exported.
    EditedNeverExported,
}

impl fmt::Display for Warn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EditedNeverExported => f.write_str("has a .pp3 but nothing in converted/"),
        }
    }
}

/// One planned action: the files it touches, relative to the argument, the
/// bytes involved, a warning when the file deserves a second look, and the
/// reason it is kept out of the run, if any.
#[derive(Default, Clone)]
pub(crate) struct Item {
    /// Photo directory, relative to the argument; groups the banner.
    pub(crate) dir: String,
    /// One printed line each.
    pub(crate) files: Vec<String>,
    /// 0 when the verb frees nothing (border, sort); the size is then left
    /// out of the summaries.
    pub(crate) size: u64,
    pub(crate) warn: Option<Warn>,
    /// The reason, already worded; an item with one does not run.
    pub(crate) kept: Option<String>,
}

impl Item {
    pub(super) fn runs(&self) -> bool {
        self.kept.is_none()
    }
}

/// A directory the verb refused to look into; the recap names it so the
/// reader knows the list is not the whole tree.
pub(crate) struct Skipped {
    pub(crate) dir: String,
    pub(crate) detail: String,
}

/// Whether the recap describes a plan or what just ran, which only changes
/// the tense of its verbs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Planned,
    Finished,
}

impl App {
    /// Prints the plan and says whether to run it: with --apply nothing is
    /// shown and the items not kept run; without it, showing the plan is the job.
    pub(crate) fn show(&self, verb: Verb, items: &[Item], skips: &[Skipped]) -> bool {
        if self.apply {
            return true;
        }

        let groups = by_directory(items);
        for group in &groups {
            if group.run.is_empty() {
                continue;
            }
            // One directory needs no header; several get a banner each so the
            // eye finds where a folder starts in a long list.
            if groups.len() > 1 {
                let banner = format!("{BANNER} {}: {}", group.dir, summary(&group.run));
                self.log.info(&self.palette.bold(&banner));
            }
            for item in &group.run {
                self.show_item(verb, item);
            }
        }

        for line in self.recap(verb, items, skips, Stage::Planned) {
            self.log.info(&line);
        }
        false
    }

    fn show_item(&self, verb: Verb, item: &Item) {
        let would = self.palette.would(&format!("would {verb}"));
        let mut warn = item.warn;
        for file in &item.files {
            match warn {
                Some(w) => {
                    self.log.info(&format!(
                        "{would} {file}  {}",
                        self.palette.gone(&format!("{BANNER} {w}"))
                    ));
                }
                None => self.log.info(&format!("{would} {file}")),
            }
            // The warning is about the item; its first file carries it.
            warn = None;
        }
    }
}

#[cfg(test)]
#[path = "show_test.rs"]
pub(crate) mod tests;

use std::fmt;

use crate::log::{BANNER, Log};

use super::{App, files, human};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Delete,
    Write,
    Move,
}

impl Verb {
    fn past(self) -> &'static str {
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
    pub(crate) files: Vec<String>,
    pub(crate) size: u64,
    pub(crate) warn: Option<Warn>,
    pub(crate) kept: Option<String>,
}

impl Item {
    fn runs(&self) -> bool {
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

impl<L: Log> App<L> {
    /// Prints the plan and says whether to run it: with --apply nothing is
    /// shown and the items not kept run; without it, showing the plan is the job.
    pub(crate) fn show(&self, verb: Verb, items: &[Item], skips: &[Skipped]) -> bool {
        if self.apply {
            return true;
        }

        let groups = by_directory(items);
        for (dir, group) in &groups {
            let run: Vec<&Item> = group.iter().copied().filter(|it| it.runs()).collect();
            if run.is_empty() {
                continue;
            }
            // One directory needs no header; several get a banner each so the
            // eye finds where a folder starts in a long list.
            if groups.len() > 1 {
                self.log.info(
                    &self
                        .palette
                        .bold(&format!("{BANNER} {dir}: {}", summary(&run))),
                );
            }
            for it in run {
                self.show_item(verb, it);
            }
        }

        for line in self.recap(verb, items, skips, Stage::Planned) {
            self.log.info(&line);
        }
        false
    }

    fn show_item(&self, verb: Verb, it: &Item) {
        let would = self.palette.would(&format!("would {verb}"));
        for (i, f) in it.files.iter().enumerate() {
            match it.warn {
                Some(warn) if i == 0 => {
                    self.log.info(&format!(
                        "{would} {f}  {}",
                        self.palette.gone(&format!("{BANNER} {warn}"))
                    ));
                }
                _ => self.log.info(&format!("{would} {f}")),
            }
        }
    }

    /// The block a reader checks before and after --apply: per directory what
    /// runs, what is kept and what was skipped, how many files deserve a look,
    /// then the totals.
    pub(crate) fn recap(
        &self,
        verb: Verb,
        items: &[Item],
        skips: &[Skipped],
        at: Stage,
    ) -> Vec<String> {
        let groups = by_directory(items);
        let (run, kept): (Vec<&Item>, Vec<&Item>) = items.iter().partition(|it| it.runs());

        let mut lines = Vec::new();
        if groups.len() > 1 || !kept.is_empty() || !skips.is_empty() {
            lines.push(self.palette.bold(&format!("{BANNER} recap")));
            lines.extend(self.recap_groups(verb, &groups, skips, at));
        }

        let flagged: Vec<&&Item> = run.iter().filter(|it| it.warn.is_some()).collect();
        if let Some(first) = flagged.first()
            && let Some(warn) = first.warn
        {
            let line = format!("{BANNER} {} flagged: {warn}", files(flagged.len()));
            lines.push(self.palette.gone(&line));
        }

        let total = if at == Stage::Finished {
            ran(verb, &run)
        } else {
            summary(&run)
        };
        let total = if kept.is_empty() {
            total
        } else {
            format!("{total}, {} kept", summary(&kept))
        };

        // Nothing to run means no hint either: --apply would do nothing.
        if run.is_empty() {
            let rest = match total.strip_prefix(&summary(&run)) {
                Some(r) => r,
                None => &total,
            };
            lines.push(self.palette.bold(&format!("nothing to do{rest}.")));
            return lines;
        }
        if at == Stage::Finished {
            lines.push(self.palette.bold(&format!("{total}.")));
            return lines;
        }
        lines.push(format!(
            "{} Re-run with --apply to {verb} them.",
            self.palette.bold(&format!("{total}."))
        ));
        lines
    }

    fn recap_groups(
        &self,
        verb: Verb,
        groups: &[(&str, Vec<&Item>)],
        skips: &[Skipped],
        at: Stage,
    ) -> Vec<String> {
        let mut width = 0;
        for (dir, _) in groups {
            width = width.max(dir.len());
        }
        for sk in skips {
            width = width.max(sk.dir.len());
        }
        let action = if at == Stage::Finished {
            verb.past().to_owned()
        } else {
            verb.to_string()
        };

        let mut lines = Vec::new();
        for (dir, group) in groups {
            let (run, kept): (Vec<&Item>, Vec<&Item>) = group.iter().partition(|it| it.runs());
            if !run.is_empty() {
                lines.push(format!("{action:<7} {dir:<width$}  {}", summary(&run)));
            }
            if !kept.is_empty() {
                let line = format!(
                    "{:<7} {dir:<width$}  {}  {}",
                    "keep",
                    summary(&kept),
                    reasons(&kept).join(", ")
                );
                lines.push(self.palette.faint(&line));
            }
        }
        for sk in skips {
            lines.push(
                self.palette
                    .faint(&format!("{:<7} {:<width$}  {}", "skip", sk.dir, sk.detail)),
            );
        }
        lines
    }
}

/// Words the total once the verb ran: "3 files deleted, 49 MB freed".
fn ran(verb: Verb, run: &[&Item]) -> String {
    let n: usize = run.iter().map(|it| it.files.len()).sum();
    let total: u64 = run.iter().map(|it| it.size).sum();
    match verb {
        Verb::Delete => format!("{} deleted, {} freed", files(n), human(total)),
        Verb::Move => format!("{} moved", files(n)),
        Verb::Write => format!("{} written", files(n)),
    }
}

/// "3 files, 49 MB", or just the count when sizes are unknown.
fn summary(items: &[&Item]) -> String {
    let n: usize = items.iter().map(|it| it.files.len()).sum();
    let total: u64 = items.iter().map(|it| it.size).sum();
    if total == 0 {
        return files(n);
    }
    format!("{}, {}", files(n), human(total))
}

/// The distinct reasons that kept these items, in order of appearance.
fn reasons<'a>(kept: &[&'a Item]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for it in kept {
        if let Some(k) = it.kept.as_deref()
            && !out.contains(&k)
        {
            out.push(k);
        }
    }
    out
}

/// Splits the items by directory, in order of appearance.
fn by_directory(items: &[Item]) -> Vec<(&str, Vec<&Item>)> {
    let mut groups: Vec<(&str, Vec<&Item>)> = Vec::new();
    for it in items {
        match groups.iter_mut().find(|(d, _)| *d == it.dir) {
            Some((_, group)) => group.push(it),
            None => groups.push((&it.dir, vec![it])),
        }
    }
    groups
}

#[cfg(test)]
#[path = "show_test.rs"]
pub(crate) mod tests;

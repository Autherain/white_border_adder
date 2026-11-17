//! The recap block: what runs, what is kept and what was skipped, per
//! directory, then the totals. Printed after the plan and again after --apply.

use crate::log::BANNER;

use super::show::{Item, Skipped, Stage, Verb};
use super::{App, files, human};

impl App {
    /// The block a reader checks before and after --apply: per directory what
    /// runs, what is kept and what was skipped, how many files deserve a look,
    /// then the totals.
    pub(crate) fn recap(
        &self,
        verb: Verb,
        items: &[Item],
        skips: &[Skipped],
        stage: Stage,
    ) -> Vec<String> {
        let groups = by_directory(items);
        let mut run = Vec::new();
        let mut kept = Vec::new();
        for group in &groups {
            run.extend_from_slice(&group.run);
            kept.extend_from_slice(&group.kept);
        }

        let mut lines = Vec::new();
        // One directory with nothing kept or skipped: the total says it all.
        if groups.len() > 1 || !kept.is_empty() || !skips.is_empty() {
            lines.push(self.palette.bold(&format!("{BANNER} recap")));
            lines.extend(self.recap_groups(verb, &groups, skips, stage));
        }
        if let Some(line) = flagged_line(items) {
            lines.push(self.palette.gone(&line));
        }
        lines.push(self.total_line(verb, &run, &kept, stage));
        lines
    }

    /// The last line: what runs or ran, what is kept, and before --apply the
    /// hint to re-run with it.
    fn total_line(&self, verb: Verb, run: &[Item], kept: &[Item], stage: Stage) -> String {
        let mut total = summary(run);
        if stage == Stage::Finished {
            total = summary_after_run(verb, run);
        }
        if !kept.is_empty() {
            total = format!("{total}, {} kept", summary(kept));
        }

        // Nothing to run means no hint either: --apply would do nothing.
        if run.is_empty() {
            // "0 files" would read oddly after "nothing to do"; what follows
            // it (the kept part) is still worth saying.
            let after_count = match total.strip_prefix(&summary(run)) {
                Some(rest) => rest,
                None => &total,
            };
            return self.palette.bold(&format!("nothing to do{after_count}."));
        }
        if stage == Stage::Finished {
            return self.palette.bold(&format!("{total}."));
        }
        format!(
            "{} Re-run with --apply to {verb} them.",
            self.palette.bold(&format!("{total}."))
        )
    }

    /// One line per directory and per action, the directory names padded to
    /// the same width so the summaries line up.
    fn recap_groups(
        &self,
        verb: Verb,
        groups: &[Group],
        skips: &[Skipped],
        stage: Stage,
    ) -> Vec<String> {
        let width = widest_dir(groups, skips);
        let mut action = verb.to_string();
        if stage == Stage::Finished {
            action = verb.past().to_owned();
        }

        let mut lines = Vec::new();
        for group in groups {
            lines.extend(self.group_lines(&action, width, group));
        }
        for skip in skips {
            let line = format!("{:<7} {:<width$}  {}", "skip", skip.dir, skip.detail);
            lines.push(self.palette.faint(&line));
        }
        lines
    }

    /// What runs in the directory, then what is kept there and why.
    fn group_lines(&self, action: &str, width: usize, group: &Group) -> Vec<String> {
        let dir = &group.dir;
        let mut lines = Vec::new();
        if !group.run.is_empty() {
            let run = summary(&group.run);
            lines.push(format!("{action:<7} {dir:<width$}  {run}"));
        }
        if !group.kept.is_empty() {
            let kept = summary(&group.kept);
            let why = reasons(&group.kept).join(", ");
            let line = format!("{:<7} {dir:<width$}  {kept}  {why}", "keep");
            lines.push(self.palette.faint(&line));
        }
        lines
    }
}

fn widest_dir(groups: &[Group], skips: &[Skipped]) -> usize {
    let mut width = 0;
    for group in groups {
        if group.dir.len() > width {
            width = group.dir.len();
        }
    }
    for skip in skips {
        if skip.dir.len() > width {
            width = skip.dir.len();
        }
    }
    width
}

fn file_count(items: &[Item]) -> usize {
    let mut count = 0;
    for item in items {
        count += item.files.len();
    }
    count
}

fn total_size(items: &[Item]) -> u64 {
    let mut bytes = 0;
    for item in items {
        bytes += item.size;
    }
    bytes
}

/// "#### 2 files flagged: <the first warning listed>", or nothing when no
/// item that runs carries a warning.
fn flagged_line(items: &[Item]) -> Option<String> {
    let mut flagged = 0;
    let mut first_warn = None;
    for item in items {
        if !item.runs() {
            continue;
        }
        let warn = match item.warn {
            Some(w) => w,
            None => continue,
        };
        if first_warn.is_none() {
            first_warn = Some(warn);
        }
        flagged += 1;
    }
    let warn = first_warn?;
    Some(format!("{BANNER} {} flagged: {warn}", files(flagged)))
}

/// Words the total once the verb ran: "3 files deleted, 49 MB freed".
fn summary_after_run(verb: Verb, run: &[Item]) -> String {
    let count = files(file_count(run));
    match verb {
        Verb::Delete => format!("{count} deleted, {} freed", human(total_size(run))),
        Verb::Move => format!("{count} moved"),
        Verb::Write => format!("{count} written"),
    }
}

/// "3 files, 49 MB", or just the count when sizes are unknown.
pub(super) fn summary(items: &[Item]) -> String {
    let count = files(file_count(items));
    let bytes = total_size(items);
    if bytes == 0 {
        return count;
    }
    format!("{count}, {}", human(bytes))
}

/// The distinct reasons that kept these items, in order of appearance.
fn reasons(kept: &[Item]) -> Vec<String> {
    let mut distinct = Vec::new();
    for item in kept {
        let why = match &item.kept {
            Some(w) => w,
            None => continue,
        };
        if !distinct.contains(why) {
            distinct.push(why.clone());
        }
    }
    distinct
}

/// The items of one photo directory, those that run apart from those kept.
pub(super) struct Group {
    pub(super) dir: String,
    pub(super) run: Vec<Item>,
    pub(super) kept: Vec<Item>,
}

/// Splits the items by directory, in order of appearance.
pub(super) fn by_directory(items: &[Item]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for item in items {
        let mut index = groups.len();
        for (i, group) in groups.iter().enumerate() {
            if group.dir == item.dir {
                index = i;
                break;
            }
        }
        let is_new_directory = index == groups.len();
        if is_new_directory {
            groups.push(Group {
                dir: item.dir.clone(),
                run: Vec::new(),
                kept: Vec::new(),
            });
        }
        if item.runs() {
            groups[index].run.push(item.clone());
        } else {
            groups[index].kept.push(item.clone());
        }
    }
    groups
}

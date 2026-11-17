//! A shot date as folder names spell it.

use jiff::civil::DateTime;

/// The three spellings of a shot date that folder names are matched against.
pub(super) struct Day {
    /// 20250804, the form wba creates.
    pub(super) compact: String,
    /// 2025-08-04, accepted in a folder that already exists.
    pub(super) iso: String,
    pub(super) year: String,
}

impl Day {
    pub(super) fn of(when: DateTime) -> Self {
        Self {
            compact: when.strftime("%Y%m%d").to_string(),
            iso: when.strftime("%Y-%m-%d").to_string(),
            year: when.strftime("%Y").to_string(),
        }
    }

    /// Whether `folder` is a day folder of this day: 20250804 or
    /// 20250804_luchon, whatever follows the date.
    pub(super) fn names(&self, folder: &str) -> bool {
        folder.starts_with(&self.compact) || folder.starts_with(&self.iso)
    }
}

pub(super) fn is_year(name: &str) -> bool {
    if name.len() != 4 {
        return false;
    }
    for byte in name.bytes() {
        if !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// The folder a photo goes to under its year, and the directories it keeps
/// below it (20250804_luchon, then day2/).
pub(super) struct Place {
    pub(super) day_folder: String,
    pub(super) below: Vec<String>,
}

/// `folders` are the directories between the root and the photo, outermost
/// first. Filed means <year>/<day folder>/, whatever comes below.
pub(super) fn already_filed(folders: &[String], day: &Day) -> bool {
    if folders.len() < 2 {
        return false;
    }
    folders[0] == day.year && day.names(&folders[1])
}

/// The dated folder the photo already sits in, at the top of the root or
/// just under a year folder: it keeps its name, and what is below it follows.
pub(super) fn dated_folder_above(folders: &[String], day: &Day) -> Option<Place> {
    if !folders.is_empty() && day.names(&folders[0]) {
        return Some(Place {
            day_folder: folders[0].clone(),
            below: folders[1..].to_vec(),
        });
    }
    // Reached only under the wrong year: `already_filed` took the right one.
    if folders.len() >= 2 && is_year(&folders[0]) && day.names(&folders[1]) {
        return Some(Place {
            day_folder: folders[1].clone(),
            below: folders[2..].to_vec(),
        });
    }
    None
}

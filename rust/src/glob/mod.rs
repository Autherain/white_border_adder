//! Globs as the user types them on --exclude, --only and --pattern.

use std::fmt;
use std::str::FromStr;

use ::glob::{MatchOptions, Pattern};

/// A glob that parsed, so a plan never meets a malformed one: clap parses
/// the flag straight into this type and rejects the bad ones itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glob {
    text: String,
    pattern: Pattern,
}

#[derive(Debug, thiserror::Error)]
#[error("{text:?}: {source}")]
pub struct Error {
    text: String,
    source: ::glob::PatternError,
}

impl FromStr for Glob {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Error> {
        let pattern = Pattern::new(text).map_err(|source| Error {
            text: text.to_owned(),
            source,
        })?;
        Ok(Self {
            text: text.to_owned(),
            pattern,
        })
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

// Go's path.Match: case folded by the callers, `*` never crosses a `/`.
const OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: false,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

impl Glob {
    #[cfg(test)]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn matches(&self, name: &str) -> bool {
        self.pattern.matches_with(name, OPTIONS)
    }
}

/// The first glob matching a photo's name without extension, a directory of
/// its path, or that whole path, so `IMG_12*` targets a photo and `*marie*`
/// targets a shoot. `rel` is the path below the argument, with `/`.
pub fn matching<'g>(rel: &str, globs: &'g [Glob]) -> Option<&'g Glob> {
    let stem = stem_of(rel);
    globs
        .iter()
        .find(|g| g.matches(stem) || stem.split('/').any(|part| g.matches(part)))
}

/// The path without the extension of its last component: "a/b.RW2" is "a/b".
fn stem_of(rel: &str) -> &str {
    let name_at = match rel.rfind('/') {
        Some(slash) => slash + 1,
        None => 0,
    };
    match rel[name_at..].rfind('.') {
        Some(dot) => &rel[..name_at + dot],
        None => rel,
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

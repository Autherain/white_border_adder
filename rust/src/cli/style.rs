use owo_colors::OwoColorize;

/// Colour marks the verb only; the path stays plain so it can be copied.
/// `main` turns colour off when stdout is not a terminal.
pub(crate) struct Palette {
    colour: bool,
}

impl Palette {
    pub(crate) fn new(colour: bool) -> Self {
        Self { colour }
    }

    pub(crate) fn would(&self, s: &str) -> String {
        if self.colour {
            s.yellow().to_string()
        } else {
            s.to_owned()
        }
    }

    pub(crate) fn done(&self, s: &str) -> String {
        if self.colour {
            s.green().to_string()
        } else {
            s.to_owned()
        }
    }

    pub(crate) fn gone(&self, s: &str) -> String {
        if self.colour {
            s.red().to_string()
        } else {
            s.to_owned()
        }
    }

    pub(crate) fn faint(&self, s: &str) -> String {
        if self.colour {
            s.dimmed().to_string()
        } else {
            s.to_owned()
        }
    }

    pub(crate) fn bold(&self, s: &str) -> String {
        if self.colour {
            s.bold().to_string()
        } else {
            s.to_owned()
        }
    }
}

#[cfg(test)]
#[path = "style_test.rs"]
mod tests;

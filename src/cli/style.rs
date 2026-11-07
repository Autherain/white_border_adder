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
        if !self.colour {
            return s.to_owned();
        }
        s.yellow().to_string()
    }

    pub(crate) fn done(&self, s: &str) -> String {
        if !self.colour {
            return s.to_owned();
        }
        s.green().to_string()
    }

    pub(crate) fn gone(&self, s: &str) -> String {
        if !self.colour {
            return s.to_owned();
        }
        s.red().to_string()
    }

    pub(crate) fn faint(&self, s: &str) -> String {
        if !self.colour {
            return s.to_owned();
        }
        s.dimmed().to_string()
    }

    pub(crate) fn bold(&self, s: &str) -> String {
        if !self.colour {
            return s.to_owned();
        }
        s.bold().to_string()
    }
}

#[cfg(test)]
#[path = "style_test.rs"]
mod tests;

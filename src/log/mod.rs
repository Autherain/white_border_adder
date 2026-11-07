//! Bare lines: info on stdout, errors on stderr, debug only when verbose.

use std::io::Write;
use std::sync::Mutex;

/// Opens every line that needs a second look, so it stands out in a long
/// list and can be grepped.
pub const BANNER: &str = "#######";

#[cfg_attr(test, mockall::automock)]
pub trait Log: Sync {
    fn debug(&self, line: &str);
    fn info(&self, line: &str);
    fn error(&self, line: &str);
}

pub struct Lines<O, E> {
    out: Mutex<O>,
    err: Mutex<E>,
    verbose: bool,
}

impl<O: Write + Send, E: Write + Send> Lines<O, E> {
    pub fn new(out: O, err: E, verbose: bool) -> Self {
        Self {
            out: Mutex::new(out),
            err: Mutex::new(err),
            verbose,
        }
    }

    /// Hands the writers back, which is how a test reads what was printed.
    #[cfg(test)]
    pub fn into_parts(self) -> (O, E) {
        let out = match self.out.into_inner() {
            Ok(w) => w,
            Err(poisoned) => poisoned.into_inner(),
        };
        let err = match self.err.into_inner() {
            Ok(w) => w,
            Err(poisoned) => poisoned.into_inner(),
        };
        (out, err)
    }
}

// A write that fails has nowhere to be reported, so it is dropped.
fn put(w: &Mutex<impl Write>, line: &str) {
    // A thread that panicked mid-write poisons the lock; the writer is
    // still usable, so the next line goes out anyway.
    let mut w = match w.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let _ = writeln!(w, "{line}");
}

impl<O: Write + Send, E: Write + Send> Log for Lines<O, E> {
    fn debug(&self, line: &str) {
        if self.verbose {
            put(&self.out, line);
        }
    }

    fn info(&self, line: &str) {
        put(&self.out, line);
    }

    fn error(&self, line: &str) {
        put(&self.err, &format!("{BANNER} {line}"));
    }
}

/// For tests that have nothing to check in the output.
#[cfg(test)]
pub struct Silent;

#[cfg(test)]
impl Log for Silent {
    fn debug(&self, _: &str) {}
    fn info(&self, _: &str) {}
    fn error(&self, _: &str) {}
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

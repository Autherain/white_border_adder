#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Fixtures for every test: a tree in memory, read back as the paths it
//! holds. Nothing touches the disk.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use vfs::{MemoryFS, VfsPath};

/// Builds the tree the fixture describes: `("paris/IMG_1264.RW2", b"raw")`.
pub fn new_tree(files: &[(&str, &[u8])]) -> VfsPath {
    let root = VfsPath::new(MemoryFS::new());
    for (path, content) in files {
        let file = root.join(path).unwrap();
        file.parent().create_dir_all().unwrap();
        file.create_file().unwrap().write_all(content).unwrap();
    }
    root
}

/// Every file under `root`, sorted, as the fixture spelled it.
pub fn files(root: &VfsPath) -> Vec<String> {
    let mut out = Vec::new();
    for entry in root.walk_dir().unwrap() {
        let p = entry.unwrap();
        if p.is_file().unwrap() {
            out.push(p.as_str().trim_start_matches('/').to_owned());
        }
    }
    out.sort();
    out
}

pub fn read(root: &VfsPath, path: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    root.join(path)
        .unwrap()
        .open_file()
        .unwrap()
        .read_to_end(&mut buf)
        .unwrap();
    buf
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;

/// A writer the test keeps a handle on after handing a clone to the log,
/// so what was printed can be read back.
#[derive(Clone, Default)]
pub struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    pub fn text(&self) -> String {
        let bytes = self.0.lock().unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

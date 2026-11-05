# One virtual filesystem, rooted at the top

Every crate under `crates/` reads and writes through `vfs::VfsPath`, the
closest thing Rust has to afero: a path that carries its filesystem, with a
`MemoryFS` for tests and a `PhysicalFS` for the real thing.

`vfs` differs from afero in two ways that shaped the code.

**Paths are relative to the filesystem root, with `/`.** A `PhysicalFS`
rooted at `photos/` cannot name `photos/../2025`, which `sort` needs when run
from inside a year folder, nor an absolute `--out`. So `main` roots the one
`PhysicalFS` at the top of the disk (`/`, or the drive on Windows) and
`locate` turns every argument into an absolute path on it. A move between a
`[source]` and `<dir>` is then a rename on the same filesystem, never a copy.
Printed paths come from `wba_path::relative`, which gives the user back the
path the way they typed the argument.

**`VfsPath` is cheap to clone but not `Ord` or `Hash`.** It is an `Arc` to
the filesystem plus an `Arc<str>`; cloning it is two pointer bumps. Sorting
and maps go through `as_str()`.

Two warts are accepted. `PhysicalFS::read_dir` panics on a file name that
is not UTF-8, which no camera produces. `is_dir()` on a missing path is
`false`, so a missing argument is reported as "not a directory" rather than
"no such file".

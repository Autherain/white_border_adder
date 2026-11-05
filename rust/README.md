# wba, in Rust

The same tool as the Go `wba` at the root of this repository, same verbs,
same flags, same output lines. One crate, one module per Go package.

```
cargo build --release          # target/release/wba
cargo test
cargo clippy --all-targets -- -D warnings
```

## Layout

| Module | Go package | What it holds |
|---|---|---|
| `src/main.rs` | `cmd/wba` | the only place that touches the OS filesystem, and the `mod` list. |
| `src/cli/` | `internal/cli` | clap verbs, `show`, output lines, exit codes. No business logic. |
| `src/border/` | `internal/border` | `plan` and `Job::run`. The only module that uses `image`. |
| `src/clean/` | `internal/clean` | `plan` and `Candidate::delete`. |
| `src/sorter/` | `internal/sorter` | `plan`, `Move::run`, the EXIF head reader. |
| `src/log/` | `internal/log` | the `Log` trait, `Lines` for real output, `MockLog` under `cargo test`. |
| `src/glob/` | `internal/glob` | `Glob`, parsed once by clap so a plan never meets a bad one. |
| `src/runeach/` | `cli/pool.go` | `run_each`: a bounded pool, results in completion order, no dependency. |
| `src/path/` | `filepath.Rel` | paths relative to the argument, stem and extension. |
| `src/testfs/` | `newTree` | the in-memory fixture every test builds, compiled only under `cargo test`. |

Every module is a directory, as a Go package is: `src/clean/mod.rs` holds
the doc comment, the types and the method that acts, `plan.rs` next to it
the planning. A module is private to the crate unless `pub`, which is Go's
`internal/` without a second crate.

The code is written to be read by someone coming from Go: `match`, `if let`
and `for` over chained combinators, `?` as the one idiom to learn. See
`docs/decisions/plain-over-clever.md`.

Tests sit next to the code they test, `plan.rs` and `plan_test.rs`, through
`#[cfg(test)] #[path = "plan_test.rs"] mod tests;`. A test module is a child
of the module it tests, so it reads private items with `use super::*`.

## What replaced what

| Go | Rust | Note |
|---|---|---|
| `afero.Fs` | `vfs::VfsPath` | One `PhysicalFS` rooted at `/`; `MemoryFS` in tests. See `docs/decisions/filesystem.md`. |
| `slog` in the context | `&dyn Log` passed explicitly | No ambient logger: a function that prints says so in its signature. |
| mockery | `mockall::automock` | `MockLog` and `MockDates`; a test states the exact lines it expects, in order. |
| `pond` | `runeach` | `std::thread::scope` and an `mpsc` channel, 40 lines. |
| `path.Match` | `glob::Pattern` | Case folded, `*` never crosses `/`, like Go. |
| `time` | `jiff` | EXIF dates parsed as civil time; mtime fallback through the system zone. |
| `x/image/draw` | `jpeg-decoder` + `fast_image_resize` + `image` | A JPEG is decoded already reduced (1/2, 1/4, 1/8), resized on SIMD, encoded by `image`. Half the CPU, a fifth of the memory; see `docs/bench.md`. |
| hand-written EXIF | hand-written EXIF | `kamadak-exif` rejects Panasonic RW2, whose TIFF magic is 0x55 instead of 42. |
| `lipgloss` | `owo-colors` | `main` turns colour off when stdout is not a terminal or `NO_COLOR` is set. |
| `-ldflags -X version` | `WBA_VERSION` at build time | `WBA_VERSION=$(git describe) cargo build --release`. |

## Where Rust changed the shape

- A verb's unit of work (`Job`, `Candidate`, `Move`) is consumed by its
  action: `job.run(o)` takes `self`, so a job cannot run twice. The result
  carries what to print, `Ok(output path)` or `Err(Failed { input, source })`.
- `kept` is an `Option<Kept>` enum with a `Display`, not an empty-string
  sentinel. `Move::to` is `Option<VfsPath>`: a kept move without a target
  cannot pretend to have one.
- `show` returns whether to apply instead of a list of indexes; each verb
  then filters its own items with `.filter(|c| c.kept.is_none())`.
- `run_each` moves the items into the workers; nothing is cloned to be
  processed. `VfsPath` itself is an `Arc`, so the clones that remain are
  pointer copies.

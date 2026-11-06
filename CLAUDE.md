# wba

## Comments

- A comment says why, never what. The code already says what it does; a
  comment that restates it is noise and gets deleted.
- No doc comment on a `pub` item unless it tells the reader something the
  name and signature do not. `/// Plans.` on `plan` is forbidden.
- Comment the surprising: a workaround, an order that matters, an invariant
  the compiler cannot check, a reference to the tool whose behaviour we
  mimic (RawTherapee writes `converted/` and `.pp3` sidecars).
- No section banners, no commented-out code, no TODO without an owner.
- Safety rules are tested, not commented. A test named after the rule is the
  documentation.

## Structure

- `src/main.rs` builds the one `PhysicalFS`, rooted at the top of the disk,
  and calls `cli::run`, nothing else. It is the only place that touches
  `std::fs` or `std::env`.
- `src/cli` owns clap flags, output lines and exit codes. It calls the
  modules below and prints; it holds no business logic. A verb builds its
  list of items, `show` prints it and says whether to apply.
- `src/border`, `src/clean` and `src/sorter` expose `plan(log, dir,
  &Options)` returning a plan, and a method on the plan's element that
  performs the action (`Job::run`, `Candidate::delete`, `Move::run`). The
  method takes `self`: an action cannot run twice. Planning never writes;
  dry-run is `plan` without the method.
- Every module reads and writes through the `vfs::VfsPath` it receives.
  See `docs/decisions/filesystem.md`.
- Output goes through the `&dyn Log` a function receives (`src/log`): info
  on stdout, errors on stderr, debug only under `--verbose`. No ambient
  logger: a function that prints says so in its signature. One message is
  one bare line; no timestamps, no levels in the text, no progress bar, no
  emoji. Colour marks the verb, never the path.
- No interactive mode: the dry run is the review step. Output is tested by
  comparing the exact lines written through `Lines` on a buffer, or by a
  `MockLog` that states the lines it expects, in order.
- Tests run on a `MemoryFS` built with `testfs::new_tree(&[(path, bytes)])`
  so a fixture reads as the tree it describes. Nothing touches the disk.
  Tests live in `<file>_test.rs` next to `<file>.rs`; see
  `docs/decisions/tests-beside-code.md`.
- Plain control flow over combinator chains; see
  `docs/decisions/plain-over-clever.md`.
- `make lint` is `cargo fmt --check` and clippy pedantic with warnings as
  errors; `make test` is `cargo test`. Both must pass before a commit.
- Decisions that need a paragraph go to `docs/decisions/<topic>.md`.

# wba

## Comments

- A comment says why, never what. The code already says what it does; a
  comment that restates it is noise and gets deleted.
- No doc comment on an exported symbol unless it tells the reader something
  the name and signature do not. `// Plan plans.` is forbidden.
- Comment the surprising: a workaround, an order that matters, an invariant
  the compiler cannot check, a reference to the tool whose behaviour we
  mimic (RawTherapee writes `converted/` and `.pp3` sidecars).
- No section banners, no commented-out code, no TODO without an owner.
- Safety rules are tested, not commented. A test named after the rule is the
  documentation.

## Structure

- `cmd/wba/main.go` builds the cobra root and calls `Execute`, nothing else.
  It is the only place that creates `afero.NewOsFs()`.
- `internal/cli` owns flags, output lines and exit codes. It calls the
  packages below and prints; it holds no business logic.
- `internal/border` and `internal/clean` expose `Plan(fs, dir, Options)`
  returning a list, and a method on the list element that performs the
  action. Planning never writes; dry-run is `Plan` without the method.
- Every package under `internal/` reads and writes through the `afero.Fs`
  it receives. No `os.ReadDir`, `os.Remove`, `os.Create` or
  `filepath.WalkDir` outside `cmd/`.
- Output is plain lines on stdout, errors on stderr, via `fmt.Fprintln`.
  No logger, no progress bar, no emoji.
- Tests run on `afero.NewMemMapFs()` built with `newTree(t, map[string]string)`
  so a fixture reads as the tree it describes. Nothing touches the disk.
- Decisions that need a paragraph go to `docs/decisions/<topic>.md`.

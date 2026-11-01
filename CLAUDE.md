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
- `internal/cli` owns flags, output lines, the interactive list and exit
  codes. It calls the packages below and prints; it holds no business logic.
  A verb asks a `picker` which planned items to run: `teaPicker` is the
  bubbletea list, `plainPicker` answers from `--apply`. Verbs never know
  which one they got.
- `internal/border` and `internal/clean` expose `Plan(fs, dir, Options)`
  returning a list, and a method on the list element that performs the
  action. Planning never writes; dry-run is `Plan` without the method.
- Every package under `internal/` reads and writes through the `afero.Fs`
  it receives. No `os.ReadDir`, `os.Remove`, `os.Create` or
  `filepath.WalkDir` outside `cmd/`.
- Output goes through the logger in the context (`internal/log`): info on
  stdout, errors on stderr, debug only under `--verbose`. One message is one
  bare line; no timestamps, no levels in the text, no progress bar, no
  emoji. Colour marks the verb, never the path.
- The bubbletea model is tested as a pure function: send `tea.KeyMsg`
  values to `Update`, assert `chosen()` and `View()`. One `teatest` run
  covers the loop. No pseudo-terminals in tests.
- Tests run on `afero.NewMemMapFs()` built with `newTree(t, map[string]string)`
  so a fixture reads as the tree it describes. Nothing touches the disk.
- Decisions that need a paragraph go to `docs/decisions/<topic>.md`.

# Rewrite prompt: cobra CLI and `clean` verb

Prompt for the Claude session that does the code. Every decision below is
taken; do not reopen it, ask only when something here is contradictory.

## Context

`golang/main.go` adds white borders to JPEG/PNG files so they fit Instagram
(1080x1080). It works but is a single flag-parsed file with batch statistics
nobody reads. The owner shoots Panasonic RAW (`.RW2`) and develops the keepers
with RawTherapee, which writes `<dir>/converted/<stem>.jpg` and leaves a
`<stem>.RW2.pp3` sidecar next to the raw. Raws that were never developed pile
up and the owner wants a verb that deletes them safely.

Models to copy for structure and tone: `~/git/sauron` (cobra tree,
`CLAUDE.md` comment rules, `.golangci.yml`, `Makefile`) and
`~/git/kubernetes-access` (README written as vocabulary, 4-line usage block,
one worked example with real input and real output).

## Target layout

```
go.mod                       module github.com/evaneecloo/white_border_adder, go 1.23
cmd/wba/main.go              cobra root + Execute, nothing else
internal/cli/                root command, shared flags (--apply, --recursive, --verbose)
internal/border/             port of golang/main.go, package API: Plan(dir, Options) ([]Job, error); Job.Run()
internal/clean/              new: Plan(dir, Options) ([]Candidate, error); Candidate.Delete()
docs/                        this file, later docs/decisions/<topic>.md if a choice needs a paragraph
Makefile                     lint, test, build (current OS only; no cross-compile targets)
.golangci.yml                sauron's file minus forbidigo and depguard rules
CLAUDE.md                    sauron's comment rules, verbatim, sections Comments and Structure
README.md                    rewritten, see below
```

Delete `golang/` once ported. `rust/` and `build/` are already gone. Keep the
two sample JPEGs at the root, the README showcase uses them.

Binary name is `wba`. Commands: `wba border`, `wba clean`, `wba version`.

## Shared behaviour

- **Dry-run by default.** Without `--apply` a verb prints what it would do,
  one line per action, then a one-line total, and exits 0. With `--apply` it
  does it and prints the same lines in the past tense. Nothing else writes
  or deletes. A verb that finds nothing to do prints `nothing to do` and exits 0.
- **Output is plain lines, no emoji, no box drawing.** `would delete
  2024-10-paris/IMG_1264.RW2` is the whole format. Paths are printed
  relative to the argument the user gave.
- **Errors.** A file that fails is printed with its error and counted; the
  verb continues and exits 1 at the end if anything failed. Do not stop on
  the first error.
- **Logging.** `fmt.Fprintln(os.Stdout/os.Stderr)` only. No logger package,
  no progress bars, no timing statistics. `--verbose` adds the skipped items
  and the reason they were skipped.
- `--recursive` walks the tree below the argument with `filepath.WalkDir`,
  skipping dot-directories and any `converted/` directory as a walk root.

## `wba border <dir>`

Port of the existing code. Keep the image math exactly (ratios, scaling with
`draw.ApproxBiLinear`, white canvas, centering). Keep the defaults:

| Flag | Default |
|---|---|
| `--width`, `--height` | 1080 |
| `--landscape-vert`, `--landscape-horiz` | 0.05, 0.03 |
| `--portrait-vert`, `--portrait-horiz` | 0.005, 0.18 |
| `--jpeg-quality` | 100 |
| `--prefix` | `bordered_` |
| `--out` | `<dir>/bordered` (relative to each photo dir when `--recursive`) |
| `--workers` | `runtime.NumCPU()` |

Drop `--batch-size`, `--separate-folder` and `--input` (the directory is the
positional argument, required). Drop every statistics type. Workers: a
`sync.WaitGroup` and a buffered channel of jobs, nothing more.

Input files: `.jpg`, `.jpeg`, `.png`, case-insensitive, non-recursive unless
`--recursive`. An output file that already exists is skipped unless
`--force`. Dry-run line: `would write bordered/bordered_IMG_1264.jpg`.

## `wba clean <dir>`

Deletes raw files that RawTherapee never developed.

Vocabulary, to reuse verbatim in the README:

- **Photo directory**: a directory that contains a `converted/`
  subdirectory. `clean` never touches a directory without one. No
  `converted/` means no sorting happened there, so nothing is a candidate.
  This is the main guard in `--recursive` mode; print such directories under
  `--verbose` as `skip <dir>: no converted/`.
- **Raw**: a file at the root of a photo directory matching `--pattern`
  (default `*.rw2`, matched case-insensitively against the file name with
  `path.Match`).
- **Stem**: the raw file name without its extension (`IMG_1264`).
- **Developed**: a raw is developed when any regular file in `converted/`
  has a name that starts with its stem, case-insensitively. This matches
  `IMG_1264.jpg`, `IMG_1264_converted.jpg`, `IMG_1264-bw.tif`. Do not match
  on extension.
- **Candidate**: a raw that is not developed and whose stem matches no
  `--exclude` glob.
- **Sidecar**: `<raw name>.pp3` next to the raw. A candidate's sidecar is
  deleted with it; a sidecar whose raw is kept is kept.

Flags:

| Flag | Default | Meaning |
|---|---|---|
| `--pattern` | `*.rw2` | which files are raws |
| `--exclude` | none, repeatable | glob on the stem; `--exclude 'IMG_12*' --exclude IMG_1300` |
| `--recursive` | false | every photo directory below `<dir>` |
| `--apply` | false | delete; otherwise print only |

Dry-run output, one candidate per line, sidecar on its own line, then the
total:

```
would delete 2024-10-paris/IMG_1264.RW2
would delete 2024-10-paris/IMG_1264.RW2.pp3
would delete 2024-10-paris/IMG_1271.RW2
3 files, 61 MB. Re-run with --apply to delete them.
```

Taking a photo out of the list is `--exclude <stem>` on the next run. No
config file, no interactive prompt, no trash directory: the dry-run default
is the safety net. Deletion is `os.Remove`, never `os.RemoveAll`.

Safety rules to test, not to comment:

- A directory without `converted/` yields zero candidates.
- A raw in `converted/` itself or in any subdirectory of the photo
  directory is never a candidate; only the root of the photo directory is
  scanned.
- The argument must exist and be a directory; otherwise exit 1 before any
  listing.
- `--exclude` applies before counting, so the total reflects what `--apply`
  would do.

## Tests

`go test ./...` with `t.TempDir()` fixtures: a photo directory with three
raws, one developed under `converted/IMG_1264.jpg`, one with a sidecar, one
excluded. Assert the exact candidate list and that dry-run leaves the tree
unchanged. For `border`, one table test on the scale/offset math with
landscape and portrait inputs; no image decoding in tests.

## README

Rewrite from scratch in the kubernetes-access style, in this order: one
sentence on what it is, the 4-line usage block, a **Vocabulary** table (photo
directory, raw, developed, candidate, sidecar), one section per verb with
its flags table, one worked example showing a tree before, the dry-run
output, and the tree after `--apply`. Keep the before/after image showcase.
No feature list with emoji, no performance tips, no license section.

## Commits

Conventional commits, no body, English. Suggested split:

1. `chore: move go module to repository root`
2. `feat: cobra command tree with border verb`
3. `feat: clean verb deleting undeveloped raws`
4. `docs: rewrite README around the two verbs`

Run `make lint` and `make test` before each commit. No AI attribution
anywhere.

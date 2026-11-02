# wba

`wba` adds white borders to photos so they fit Instagram, and deletes the
Panasonic raws that RawTherapee never developed.

```
wba border <dir>            # write <dir>/bordered/bordered_<name>.jpg for every JPEG and PNG
wba clean <dir>             # delete raws without a developed copy in <dir>/converted/
wba sort <dir>              # file photos under <dir>/<year>/<day folder> by shot date
wba <verb> <dir> --apply    # verbs only print what they would do until you add --apply
```

|              Original               |                 With borders                 |
| :---------------------------------: | :------------------------------------------: |
| ![Original](IMG_1264_converted.jpg) | ![Bordered](bordered_IMG_1264_converted.jpg) |

## Vocabulary

| Term | Meaning |
|---|---|
| **Photo directory** | A directory that contains a `converted/` subdirectory, which is where RawTherapee writes developed photos. `clean` never touches a directory without one: no `converted/` means no sorting happened there, so nothing is a candidate. |
| **Raw** | A file at the root of a photo directory matching `--pattern` (default `*.rw2`, case-insensitive). |
| **Stem** | The raw file name without its extension: `IMG_1264`. |
| **Developed** | A raw is developed when any file in `converted/` has a name that starts with its stem, case-insensitively. `IMG_1264.jpg`, `IMG_1264_converted.jpg` and `IMG_1264-bw.tif` all develop `IMG_1264.RW2`. The extension does not matter. |
| **Candidate** | A raw that is not developed and that no `--exclude` glob keeps, by its stem or by a directory of its path. |
| **Sidecar** | The `<raw name>.pp3` file RawTherapee leaves next to the raw. A candidate's sidecar is deleted with it; the sidecar of a raw that is kept is kept. |

## How a verb runs

Every verb plans first and prints the plan, one `would ...` line per file.
With `--recursive` each directory opens with a `####### <dir>: <count>,
<size>` banner. The plan ends with a recap: one line per directory saying
what goes and what `--exclude` keeps, with sizes, then the totals.

```
####### recap
delete  20260620_toulouse       64 files, 633 MB
keep    20260701_portait_marie  21 files, 317 MB  --exclude *marie*
delete  20260810_suisse         14 files, 156 MB
keep    20260810_suisse         99 files, 1.8 GB  --exclude _13409*
skip    20260811_liechtenstein  124 raws, 2.4 GB, no converted/: not sorted yet
####### 71 files flagged: has a .pp3 but nothing in converted/
182 files, 2.2 GB, 169 files, 2.9 GB kept. Re-run with --apply to delete them.
```

A `skip` line is a directory `clean` refused to look into because it has no
`converted/`: sort it in RawTherapee first. When nothing would run, the last
line says `nothing to do` instead of inviting you to `--apply`.

Nothing is written or deleted until you add `--apply`, which does the
plan, prints the same lines in the past tense and the same recap. Errors
are printed with the `#######` banner on stderr.

`clean` marks a raw that has a `.pp3` sidecar but nothing in `converted/`
with `####### has a .pp3 but nothing in converted/`: you edited it in
RawTherapee and never exported it. Look before you `--apply`, or keep it
with `--exclude <stem>`.

| Flag | Default | Meaning |
|---|---|---|
| `--apply` | false | Do it. Without this flag a verb only prints what it would do. |
| `--recursive` | false | Walk the tree below `<dir>`, skipping dot-directories. `clean` also skips `converted/`, which holds no raws; `border` enters it, since that is where the exports are, and skips only its own `bordered/`. |
| `--verbose` | false | Also print skipped items and why. |

A file that fails is printed on stderr with its error, the verb goes on, and
the exit code is 1 at the end. A verb with nothing to do prints `nothing to
do` and exits 0.

## wba border <dir>

Writes a copy of every `.jpg`, `.jpeg` and `.png` at the root of `<dir>` on
a white canvas, centred, scaled to leave the border ratios below.

| Flag | Default | Meaning |
|---|---|---|
| `--width`, `--height` | 1080 | Canvas size in pixels. |
| `--landscape-vert`, `--landscape-horiz` | 0.05, 0.03 | Border ratios for landscape photos, as a fraction of the canvas. |
| `--portrait-vert`, `--portrait-horiz` | 0.005, 0.18 | Border ratios for portrait photos. |
| `--jpeg-quality` | 100 | JPEG output quality. PNG in gives PNG out. |
| `--prefix` | `bordered_` | Prefix of the output file name. |
| `--out` | `<dir>/bordered` | Output directory. Relative to each photo directory with `--recursive`. |
| `--force` | false | Overwrite an output file that already exists; otherwise it is skipped. |
| `--workers` | number of CPUs | Photos processed at once. |

```
$ wba border 2024-10-paris
would write bordered/bordered_IMG_1264.jpg
would write bordered/bordered_IMG_1271.jpg
2 files. Re-run with --apply to write them.
```

## wba clean <dir>

Deletes the raws of `<dir>` that RawTherapee never developed.

| Flag | Default | Meaning |
|---|---|---|
| `--pattern` | `*.rw2` | Glob on the file name that marks a raw. |
| `--exclude` | none | Glob that keeps a raw when it matches its stem or any directory of its path below `<dir>`. Repeatable: `--exclude 'IMG_12*' --exclude IMG_1300 --exclude '*marie*'`. |
| `--only` | none | The mirror: when given, only the raws matching one of its globs go, the rest is kept. `--only '*suisse*'` cleans one shoot out of a year. `--exclude` wins when both match. |

Taking a photo out of the list is `--exclude <stem>` on the next run, a
whole shoot is `--exclude '<directory>'`, and cleaning one shoot only is
`--only '<directory>'`. There is no config file, no prompt and no trash
directory: the dry-run default is the safety net.

### Worked example

Before:

```
2024-10-paris/
├── IMG_1264.RW2
├── IMG_1264.RW2.pp3
├── IMG_1271.RW2
├── IMG_1280.RW2
├── IMG_1280.RW2.pp3
└── converted/
    └── IMG_1280.jpg
```

```
$ wba clean 2024-10-paris
would delete IMG_1264.RW2
would delete IMG_1264.RW2.pp3
would delete IMG_1271.RW2
3 files, 49 MB. Re-run with --apply to delete them.

$ wba clean 2024-10-paris --apply
deleted IMG_1264.RW2
deleted IMG_1264.RW2.pp3
deleted IMG_1271.RW2
3 files deleted, 49 MB freed.
```

After:

```
2024-10-paris/
├── IMG_1280.RW2
├── IMG_1280.RW2.pp3
└── converted/
    └── IMG_1280.jpg
```

`IMG_1280.RW2` stays because `converted/IMG_1280.jpg` starts with its stem,
and its sidecar stays with it.

## wba sort <dir> [source...]

Files every photo under `<dir>/<year>/<day folder>` by the date it was shot.
Run it on the directory that holds your `2025/` and `2026/` folders, or from
inside one of them (a `<dir>` named like a year is searched alone, and photos
of another year leave it for that year's folder); with no `[source]` it re-files everything below `<dir>` (use `--recursive`), which
fixes photos moved into the wrong day folder. Give a `[source]` (a dump of
unsorted photos, inside or outside `<dir>`) to file only what is in it.

The date is the EXIF date the camera wrote in the file, read from its first
256 kB; a file without one falls back to its modification time. The day
folder is, in order: the dated folder the photo already sits in when its date
is right (it stays); the folder of that day already under the year, so
`20260620_toulouse` keeps its name; else a new `20260620`. Several folders
for one day keep the photo where it is and list it in the recap.

`--exclude <glob>` (repeatable) leaves alone, without even reading them, the
photos whose name without extension matches, or that have a directory of their
path matching: `--exclude 'DSC_*'` for old Nikon numbering, `--exclude
'*nikon*'` for a folder. `--verbose` prints what it left out.

Raws, JPEG and PNG move. A raw's `.pp3` and its developed copies in
`converted/` (`IMG_1264.jpg`, `IMG_1264-2.png` for `IMG_1264.RW2`, but not
`IMG_12645.jpg`) go with it into `<day folder>/converted/`. `converted/` and
`bordered/` are never scanned as photos and `bordered/` never moves. Nothing is overwritten, emptied folders
stay.

```
$ wba sort ~/photos ~/photos/dump --recursive
would move dump/IMG_1.RW2 -> 2026/20260620_toulouse/IMG_1.RW2
would move dump/IMG_1.RW2.pp3 -> 2026/20260620_toulouse/IMG_1.RW2.pp3
2 files. Re-run with --apply to move them.
```

## Install

With Go installed, on any OS:

```
go install github.com/evaneecloo/white_border_adder/cmd/wba@latest
```

Or take `wba-windows-amd64.exe`, `wba-linux-amd64` or `wba-darwin-arm64`
from the latest GitHub release.

### Windows

1. Download `wba-windows-amd64.exe` from the release page, rename it to
   `wba.exe` and put it in a folder on your `PATH`, for example
   `C:\Users\<you>\bin`. To add that folder to `PATH`, open
   *Edit the system environment variables* and append it to the user `Path`.
2. Open PowerShell or Windows Terminal in the photo directory (Shift +
   right click, *Open PowerShell window here*) and run the verbs on `.`:

```
wba clean .             # read the list, nothing is deleted
wba clean . --apply     # delete it
wba border . --apply
```

Paths work with either slash: `wba clean D:\photos\2024-10-paris` and
`wba clean D:/photos/2024-10-paris` are the same.

### From source

```
make build            # ./wba for the current OS
make build-windows    # ./wba.exe, from Linux or macOS
make install          # ~/.local/bin/wba plus zsh and bash completion files
make test
make lint             # needs golangci-lint v2
```

### Shell completion

`wba completion zsh`, `bash`, `fish` or `powershell` prints the script.
`make install` writes the zsh and bash ones next to the binary under
`~/.local/share`; for zsh, `~/.local/share/zsh/site-functions` must be in
`fpath` before `compinit`. Since the binary generates the script, a new
build followed by `make install` is all it takes after a change. For
PowerShell on Windows, add to your `$PROFILE`:

```
wba completion powershell | Out-String | Invoke-Expression
```

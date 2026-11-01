# wba

`wba` adds white borders to photos so they fit Instagram, and deletes the
Panasonic raws that RawTherapee never developed.

```
wba border <dir>            # write <dir>/bordered/bordered_<name>.jpg for every JPEG and PNG
wba clean <dir>             # delete raws without a developed copy in <dir>/converted/
wba <verb> <dir> --apply    # verbs only print what they would do until you add --apply
wba <verb> <dir> --recursive --verbose
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
| **Candidate** | A raw that is not developed and whose stem matches no `--exclude` glob. |
| **Sidecar** | The `<raw name>.pp3` file RawTherapee leaves next to the raw. A candidate's sidecar is deleted with it; the sidecar of a raw that is kept is kept. |

## Shared flags

| Flag | Default | Meaning |
|---|---|---|
| `--apply` | false | Do it. Without this flag a verb prints one `would ...` line per action and a total, and changes nothing. |
| `--recursive` | false | Walk the tree below `<dir>`, skipping dot-directories and `converted/`. |
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
| `--exclude` | none | Glob on the stem to keep. Repeatable: `--exclude 'IMG_12*' --exclude IMG_1300`. |

Taking a photo out of the list is `--exclude <stem>` on the next run. There is
no config file, no prompt and no trash directory: the dry-run default is the
safety net.

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
2. Open PowerShell in the photo directory (Shift + right click, *Open
   PowerShell window here*) and run the verbs on `.`:

```
wba clean .
wba clean . --apply
wba border . --apply
```

Paths work with either slash: `wba clean D:\photos\2024-10-paris` and
`wba clean D:/photos/2024-10-paris` are the same.

### From source

```
make build            # ./wba for the current OS
make build-windows    # ./wba.exe, from Linux or macOS
make test
make lint             # needs golangci-lint v2
```

# A TUI to review a plan, beside the CLI

Status: decided with Etienne on 2026-11-17, not implemented. The work order
is in `docs/tui-handoff.md`.

`wba` with no verb opens a full-screen review of a plan: browse it, take
files out, look at them, apply. `wba <verb> <dir>` stays exactly as it is,
bare lines and `--apply`, for scripts and for the existing tests.

The TUI is a second front end next to `src/cli`. It calls the same
`plan(log, dir, &Options)` and the same `Job::run`, `Candidate::delete` and
`Move::run`, and holds no business logic.

## Why

- A recursive `clean` over a year of shoots prints hundreds of lines; taking
  one file out means re-running with one more `--exclude`.
- `border` has eight numeric flags whose effect is only visible once the
  files are written.
- A raw cannot be judged by its name: a preview next to the line settles it.

## Layout

Stacked panels on the left, one large panel on the right, as lazygit does.
The verb is a tab; the panels stay, their content changes.

`clean` and `sort`:

```
 wba  ~/photos                                      [1 clean]  2 sort   3 border
┌ Directories ─────────────────┐┌ 20260810_suisse ───────────┐┌ Preview ─────────┐
│ delete 20260620_toulouse  64 ││ [x] _1340801.RW2    24 MB  ││                  │
│ keep   20260701_marie     21 ││ [x] _1340802.RW2    25 MB  ││   (thumbnail)    │
│>delete 20260810_suisse    14 ││ [ ] _1340901.RW2    24 MB  ││                  │
│ skip   20260811_liecht.  124 ││>[!] _1340950.RW2    26 MB  ││                  │
│                              ││                            ││ _1340950.RW2     │
│                              ││                            ││ pp3, not exported│
└──────────────────────────────┘└────────────────────────────┘└──────────────────┘
 181 files, 2.2 GB to delete · 170 kept, 2.9 GB             cache: 212/214 dirs
 tab verb   space keep/delete   o options   enter apply   q quit
```

- Directories is today's recap, one line per directory, same words
  (`delete`, `keep`, `skip`).
- `[x]` runs, `[ ]` is kept, `[!]` runs and carries a `Warn`
  (`has a .pp3 but nothing in converted/`).
- For `sort` the middle panel reads `from -> to`.

`border`, where the options are the point and stay on screen:

```
 wba  ~/photos/20260810_suisse                     1 clean   2 sort  [3 border]
┌ Files 2/3 ─────────────────┐┌ Preview  IMG_1271.jpg  landscape ─────────────┐
│ [x] IMG_1264.jpg           ││                                               │
│>[x] IMG_1271.jpg           ││   ┌───────────────────────────────────────┐   │
│ [ ] IMG_1280.jpg           ││   │                                       │   │
├ Preset ────────────────────┤│   │  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │   │
│ (•) square    1080 x 1080  ││   │  ░░░░░░░░░░░░ photo ░░░░░░░░░░░░░░░░  │   │
│ ( ) portrait  1080 x 1350  ││   │  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │   │
│ ( ) story     1080 x 1920  ││   │                                       │   │
├ Borders ───────────────────┤│   └───────────────────────────────────────┘   │
│ landscape  vert  [ 0.050 ] ││                                               │
│            horiz [ 0.030 ] ││  canvas 1080 x 1080   photo 1015 x 676        │
│ portrait   vert  [ 0.005 ] ││  top/bottom 202 px    left/right 32 px        │
│            horiz [ 0.180 ] ││                                               │
│ quality          [  100  ] ││                                               │
└────────────────────────────┘└───────────────────────────────────────────────┘
 2 files to write            wheel or +/- to change a value            enter apply
```

- The pixel line under the preview comes from `border::render::layout`,
  which is pure. It is the truth: a 0.005 border is 5 px and cannot be seen
  in a terminal thumbnail. The figures above are `layout` on a 6000 x 4000
  photo with the default ratios.
- The preview is recomputed on every value change. Only the placement is
  redone; the decoded thumbnail comes from the cache
  (`docs/decisions/cache.md`).
- Presets set `width` and `height` in one key. They are new: the CLI has
  no such flag.
- `--prefix`, `--out`, `--force`, `--workers` are rare and live in the `o`
  panel, as `--pattern`, `--exclude`, `--only` do for `clean`.

`enter` opens the confirmation that stands for `--apply`:

```
┌ Apply ────────────────────────────────┐
│ delete 181 files, 2.2 GB              │
│ 71 flagged: .pp3 but not exported     │
│                                       │
│ y apply    n back                     │
└───────────────────────────────────────┘
```

## Keys and mouse

Every action has a key. The mouse only scrolls.

| Key | Action |
|---|---|
| `tab`, `1` `2` `3` | switch verb |
| arrows, `h` `j` `k` `l` | move inside and between panels |
| `space` | keep or run the file under the cursor |
| `a` | keep or run the whole directory |
| `r` | toggle `--recursive` and plan again |
| `/` | filter the file list |
| `o` | options panel of the verb |
| `+` `-`, or digits | change the value under the cursor |
| `enter`, then `y` | apply |
| `q`, `esc` | quit, or close the panel on top |

| Mouse | Action |
|---|---|
| wheel in a list | same as up and down |
| wheel on a value | same as `+` and `-` |

The wheel is free: `crossterm` reports it as an event like a key, handled
by the focused panel.

Clicks are out. `ratatui` has no clickable widget, and the crates that add
them (`ratatui-interact`, `tuirealm`) have about 40,000 downloads a
quarter against 27 million for `ratatui`. Etienne's rule: clicks if a
well-used library gives them, none if they must be written here. Look
again if `ratatui` grows them.

Dragging the photo edge in the preview was rejected too: one terminal cell
is about 20 px of canvas, too coarse to set a border.

## How it fits the code

- `src/tui/` sits beside `src/cli/` with the same rule: no business logic.
- `Item`, `Skipped`, `by_directory` and `summary` in `src/cli/show.rs`
  already are the model the Directories panel needs. They move to a module
  both front ends use, so the recap words cannot drift apart.
- A plan is computed on a worker thread and sent over a channel; the UI
  thread never blocks on the filesystem. Only the plan of the open tab is
  computed, the others on first visit.
- A toggle lives in the TUI's state, next to the plan. Apply runs the
  checked elements through `run_each` and the `self`-taking methods, then
  plans again.
- The TUI passes its own `Log` to `plan` and the actions; its lines feed a
  panel. Nothing is written to stdout while the screen is up.
- State changes are a plain function from state and event to state, tested
  without a terminal. Rendering is tested on ratatui's `TestBackend` by
  comparing the exact lines of the buffer, on a `MemoryFS` from `testfs`.

## Rules of `CLAUDE.md` this changes

To be edited in the commit that lands the TUI, not before:

- "No interactive mode: the dry run is the review step" becomes: the CLI
  has no prompt; the TUI is the interactive review and applies only after
  `enter` then `y`.
- "no progress bar" keeps holding for CLI lines. The TUI may show a
  `planning 212/214 dirs` counter.
- Structure gains `src/tui` and `src/cache`. `src/main.rs` stays the only
  place that touches `std::fs` and `std::env`: it resolves the cache
  directory and hands it over as a `VfsPath`.

## Crates

One library, `ratatui`, and two widgets it does not have. Popularity and
upkeep were checked on GitHub and crates.io on 2026-11-17; the APIs were
not, read the docs of each and check that the widget crates match the
`ratatui` version.

| Need | Crate |
|---|---|
| Screen, layout, lists, tabs, borders | `ratatui` |
| Keys, wheel, raw mode | `crossterm`, the backend `ratatui` uses by default |
| Terminal restored on exit and on panic | `ratatui::init` and `ratatui::restore` |
| Image in a panel | `ratatui-image`: kitty, sixel or iTerm2 graphics, half-block fallback |
| Text fields: the `/` filter, a glob, a number | `ratatui-textarea`, kept by the `ratatui` organisation |
| Render tests | `ratatui`'s `TestBackend` |

| Crate | Version | Downloads, last 90 days | Last release |
|---|---|---|---|
| `ratatui` | 0.30.2 | 27 M | 2026-06-19 |
| `ratatui-image` | 11.1.0 | 0.8 M | 2026-10-18 |
| `ratatui-textarea` | 0.9.4 | 0.8 M | 2026-10-31 |

`ratatui` has 23,000 stars and no text input of its own.
`ratatui-textarea` was preferred to `tui-input` (0.7 M downloads, 207
stars) because the `ratatui` organisation maintains it.

A checkbox or a radio line is a list item whose text starts with `[x]` or
`(•)`: no widget needed.

Not needed: an async runtime (a worker thread and a channel are enough), a
fuzzy matcher (the filter reuses `glob::Glob`), a component framework on
top of `ratatui`.

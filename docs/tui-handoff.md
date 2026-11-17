# TUI and cache: handoff

Written on 2026-11-17 after a design discussion with Etienne. No code
exists yet. Read these first, in this order:

1. `CLAUDE.md`: comment, structure and test rules. They are strict.
2. `docs/decisions/tui.md`: what the TUI is, its mockups and keys.
3. `docs/decisions/cache.md`: what is cached, the keys, the location.
4. `src/cli/show.rs` and `src/cli/clean.rs`: how a verb turns a plan into
   `Item`s and a recap today.

## What Etienne asked for

- `wba` alone opens a TUI; the verb is chosen inside it.
- A lazygit-like layout, "good enough, not perfect".
- The mouse if a well-used library gives it, not if it must be written
  here: so the wheel only, no clicks. See `docs/decisions/tui.md`.
- Well-used, maintained crates rather than home-made code; the choices and
  their figures are in the Crates section of both decision docs.
- `border` options always visible, with a preview of the result.
- A cache so that recursive runs on large trees stop recomputing
  everything.
- Thumbnails of the photos.
- Reference for the look: <https://github.com/persiyanov/herdr-reviewr>,
  which nobody opened during the discussion. Look at it before drawing.
  lazygit, btop, yazi and superfile were cited from memory.

## Steps

Each step is one branch and one PR, leaves `make lint` and `make test`
green, and is useful on its own. Conventional commits, no commit body.

**0. Measure.** Nothing below is justified until this is done, on the real
photo tree:

```
findmnt -T <dir>                              # FSTYPE: nfs or cifs is a network mount
time wba clean <dir> --recursive              # run twice, compare "real"
strace -c -f wba clean <dir> --recursive      # "calls" column, statx and getdents64 rows
```

A fast second run means cold disk or network: the cache helps. Two slow
runs with few system calls means the time is in our code: fix that
instead. Write the figures at the top of `docs/decisions/cache.md`.

**1. Listing cache, no UI.** `src/cache`, wired in `main.rs`, with
`--no-cache`. The CLI gets faster and its output does not change by one
character: the existing output tests are the proof. Done when the second
`time wba clean <dir> --recursive` is measurably faster and the figure is
in the decision doc.

**2. Shared review model.** Move `Item`, `Skipped`, `Warn`, `by_directory`
and `summary` out of `src/cli/show.rs` to a module both front ends use.
Pure move, no behaviour change.

**3. TUI skeleton, `clean` only, keyboard only.** `Verb` becomes optional
in `src/cli/mod.rs`; no verb on a terminal opens the TUI on the current
directory, no verb without a terminal prints the help as today. Three
panels without preview, toggle, confirmation, apply, re-plan. Tested on
`TestBackend` and `MemoryFS`.

**4. `sort` and `border` tabs.** `border` gets the Preset and Borders
panels and the pixel line from `border::render::layout`, still without an
image.

**5. Previews and the thumbnail cache.** JPEG and PNG first, reusing the
reduced decode of `border::decode_jpeg`. Raws after, see below.

**6. Docs.** README section, the `CLAUDE.md` edits listed in
`docs/decisions/tui.md`, the Code table.

## Traps

- **A raw that `clean` lists has no developed copy by definition**, so its
  preview must come from the raw. `image` does not decode `.RW2`. Panasonic
  raws embed a JPEG (tag `JpgFromRaw`, 0x002E, as exiftool names it); the
  TIFF reader in `src/sorter/exif.rs` can find a tag. Check on a real file
  first: `exiftool -b -JpgFromRaw IMG.RW2 > preview.jpg`.
- **Graphics inside herdr are untested.** Etienne works in herdr, a
  terminal multiplexer. If it does not pass kitty or sixel graphics
  through, previews fall back to half-blocks. Test before step 5.
- **Capturing the wheel takes over text selection** in most terminals;
  shift+drag usually still selects. Say so in the README.
- **The terminal must be restored on panic and on every exit path**, or the
  shell is left in raw mode.
- **`src/main.rs` is the only file allowed to touch `std::env`.** The cache
  directory and "is stdout a terminal" are resolved there and passed down.
- **The plans log skipped files at debug level only.** A file `border`
  skips because its output exists is not in the plan, so the TUI cannot
  list it unless the plan starts returning it.

## Open questions for Etienne

Defaults are what to build if he does not answer.

1. Which tab opens first? Default: `clean`. "Last used" needs a state
   file, and the README says there is no config file.
2. Are the presets wanted, and with which sizes? Default: square
   1080 x 1080, portrait 1080 x 1350, story 1080 x 1920.
3. Can a file kept by `--exclude` be checked again by hand? Default: no,
   it is shown faint with its reason; change the glob in the `o` panel.
4. The cache keys on name, size and mtime rather than on a hash of the
   content, which is what he first asked for. The reason is in
   `docs/decisions/cache.md`. Default: as written there.
5. `wba tui <dir>` to open the TUI elsewhere than the current directory,
   rather than `wba <dir>`, which a directory named `clean` would break.
   Default: `wba tui <dir>`.

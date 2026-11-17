# A cache for listings, shot dates and thumbnails

Status: proposed on 2026-11-17, not implemented, and step 0 of
`docs/tui-handoff.md` (measure first) can still change it.

A recursive `clean`, `sort` or `border` on a large tree is slow. The cache
makes a second run, and every re-plan in the TUI, skip what has not
changed.

## What is slow, as far as the code tells

Not measured yet. Reading the code, the likely cost is one or two
filesystem calls per file, which a network mount turns into round trips:

- `clean::plan::entries` calls `metadata()` on every entry of every
  directory.
- `border::plan::Planner::scan` calls `is_dir()` then `is_file()` on every
  entry.
- `sorter::exif::shot_date` reads the first 256 kB of every photo.

`vfs` hides the file type that the OS returns for free with a directory
listing, so these calls cannot be removed from inside a plan.

## Three things are cached

| What | Key | Invalidated when |
|---|---|---|
| Directory listing: names, kinds, sizes, mtimes | directory path | the directory's mtime changed |
| Shot date of a photo | file name, size, mtime | never: another key is another file |
| Thumbnail of a photo | file name, size, mtime | never, same reason |

**Listings, not plans.** A plan depends on `--exclude`, `--pattern` and the
rest; a listing does not. Planning from listings held in memory is fast, so
one cache serves the three verbs and every option change in the TUI.

**A directory's mtime instead of a hash of its content.** Hashing a
listing needs the listing, which is the cost to avoid. The mtime of a
directory changes when an entry is added, removed or renamed, and costs one
call per directory instead of one per file. It does not change when a file
is rewritten in place: a cached size can then be stale, which only affects
the sizes printed. It does not change when a subdirectory's content does,
so every directory is checked on its own.

**A directory modified in the last two seconds is never trusted.** A
second change inside the same mtime tick would be missed; git treats its
index the same way.

**Name, size and mtime instead of a hash of the file.** `sort` moves a file
without changing any of the three, so dates and thumbnails survive a move,
and the key costs nothing: it comes from the listing. A hash of the first
256 kB was considered; reading them is the round trip the cache exists to
save. The three values are hashed with SHA-256 only to make a file name.

## Where

Outside the photo tree: `clean`, `sort` and `border` walk it, it may be a
slow or read-only mount, and a cache must be safe to delete.

- Linux: `$XDG_CACHE_HOME/wba`, else `~/.cache/wba`
- macOS: `~/Library/Caches/wba`
- Windows: `%LOCALAPPDATA%\wba`

`main.rs` resolves it, being the only place that reads `std::env`, and
hands a `VfsPath` on the one `PhysicalFS`.

```
~/.cache/wba/
├── listings/<sha256 of the directory path>
├── dates/<sha256 of name, size, mtime>
└── thumbs/<sha256 of name, size, mtime>.jpg
```

## Rules

- A missing, unreadable or unknown-version entry is a miss, never an error.
- An entry is written to a temporary name then renamed, so a killed run
  leaves no half-written file.
- A deletion, move or write done by wba drops the listing of the
  directories it touched before the next plan.
- `--no-cache` plans from the disk and writes nothing. Deleting the
  directory is always safe.
- A thumbnail is the decoded photo, reduced, without border: the border
  options never invalidate it.

## Preferred shape, to be confirmed by the implementer

A `vfs::FileSystem` that wraps the real one and answers `read_dir` and
`metadata` from the listings: `read_dir(dir)` stats the directory, compares
the mtime, returns the cached names; `metadata(file)` is answered from the
listing of its parent; a write goes through and drops the parent's entry.
The plans would not change at all, which is what
`docs/decisions/filesystem.md` set up. If the `FileSystem` trait makes this
awkward, the fallback is a `cache` module the plans call explicitly.

## Crates

The keys and the invalidation above are ours; storage is not worth
writing. Popularity and upkeep were checked on GitHub and crates.io on
2026-11-17; the APIs were not, read the docs of each before adding it.

| Need | Crate | What it replaces |
|---|---|---|
| Store | `redb`: pure Rust key-value store, one file, atomic commits | `listings/` and `dates/` become two tables in `~/.cache/wba/cache.redb`, keyed by the tuple itself; `thumbs/` stays a directory of files |
| Cache directory per OS | `dirs` (`cache_dir`) | the hand-written XDG, macOS and Windows lookup |
| Encoding a listing as a value | `serde` with `postcard` | a home-made line format and its version header |

| Crate | Version | Downloads, last 90 days | Last release |
|---|---|---|---|
| `redb` | 4.6.0 | 7.0 M | 2026-11-13 |
| `dirs` | 7.0.0 | 113 M | 2026-09-05 |

`redb` is <https://github.com/cberner/redb>, 4,900 stars. It is not
`reddb`, a different and small project. `directories` was dropped: its
repository is archived; `dirs` moved to Codeberg and is still released.

Why `redb` over the others:

- It has an in-memory backend, so tests keep the rule that nothing touches
  the disk.
- `cacache` is made for blobs addressed by content, which fits thumbnails
  but not listings, and only writes to the real disk.
- `rusqlite` and `heed` bring C code into a build that has none.
- `sled` is no longer maintained. `moka` and `cached` are in-memory caches
  and do not survive the process.

Thumbnails stay plain files, named by SHA-256 and written to a temporary
name then renamed. `redb` is on disk the largest of the stores its own
benchmark compares, and the author of
<https://rust-glancer.github.io/blog/persistent-incrementality-in-rust/>
found it too heavy for many blobs and kept it as an index only. Listings
and dates are small values, which is what it is good at.

The benchmark in the `redb` README runs on millions of entries and
gigabytes. This cache holds a few thousand listings and a few tens of
thousands of dates: every store in that table is fast enough, so speed did
not decide. `lmdb` is the fastest there and is C code.

An incremental computation framework (`salsa`, or Ajika from the article
above) was considered and left out. They track a deep graph of queries
that depend on each other; here the graph has one level, a plan computed
from listings, and that step is fast in memory. What is kept from the
article: the cache is disposable, a schema version that does not match
resets it, and it never makes wba fail.

Two consequences to accept:

- `redb` opens its file with `std::fs`, outside `vfs`. `main.rs` opens the
  database and hands it down, so it stays the only place that touches the
  disk; `docs/decisions/filesystem.md` gets a line for this exception.
- One process holds the file at a time. A second `wba` that fails to open
  it runs without cache instead of failing.

## Not verified

- That the stat calls are the cost: see step 0 of the handoff.
- That directory mtimes are reliable on the mount in use (NFS and SMB
  usually are).
- That `MemoryFS` reports a `modified` time; if not, tests need a seam like
  `sorter::Dates`.

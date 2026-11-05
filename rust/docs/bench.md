# Go vs Rust, measured

One machine, 14 cores, warm page cache, `/usr/bin/time -f "%e %U %M"`,
best of three. Go built with `make build` at the repository root, Rust with
`make build` here. Same output on every verb, checked with `md5sum`.

| Verb | Fixture | Go wall | Rust wall | Go user | Rust user | Go RSS | Rust RSS |
|---|---|---|---|---|---|---|---|
| `border --apply --workers 14` | 40 JPEG 3457x5194 | 2.7 s | 1.2 s | 29.4 s | 12.0 s | 1.2 GB | 320 MB |
| `border --apply --workers 1` | same | 22.5 s | 9.2 s | 22.3 s | 10.1 s | 139 MB | 30 MB |
| `clean --recursive` (dry run) | 200 dirs x 100 raws | 0.20 s | 0.13 s | 0.31 s | 0.19 s | 23 MB | 14 MB |
| `sort --recursive` (dry run) | 5000 raws with EXIF, 300 kB each | 0.45 s | 0.18 s | 1.15 s | 0.11 s | 33 MB | 18 MB |
| binary | | 6.7 MB | 2.7 MB | | | | |

What the numbers say:

- `border` is CPU bound on decode, resize and encode. Per photo on one
  core: Go decodes the full 18 Mpx (54 MB) and samples two pixels per output
  pixel; Rust asks `jpeg-decoder` for a picture no larger than needed (the
  inverse DCT is cut at 1/2, 1/4 or 1/8, 3 MB here), then resizes on SIMD
  lanes with `fast_image_resize`. Measured phases with the `image` crate
  alone: decode 180 ms, resize 300 ms, encode 52 ms; with the reduced
  decode and SIMD resize: 145 ms, 15 ms, 52 ms. Memory follows: a worker
  holds a 3 MB picture instead of a 54 MB one.
- The output differs from Go's by 3 % RMSE: Go's `ApproxBiLinear` samples,
  the convolution averages, so Rust's copy is smoother, not shifted.
- `clean` and `sort` are IO bound. Rust reads the 5000 EXIF heads with a
  tenth of the CPU: no garbage collector, no goroutine scheduler, and the
  bytes stay in one `Vec` from the read to the parse.
- Memory at rest is half of Go's: no runtime, no GC headroom.

Before the reduced decode, Rust `border` was at 2.6 s and 1.5 GB: same
wall as Go, 15 % less CPU, more memory for the RGBA canvas. The first run
of this benchmark had it at 19 s: `run_each` held
the queue's mutex while the worker ran, because a guard taken inside a
`while let` condition lives for the whole loop body. The pool test
`workers_run_at_the_same_time` now keeps that from coming back.

To rerun: build both, make the fixtures (40 copies of the sample JPEG,
200 shoot directories of 100 small raws, 5000 raws with a 128-byte EXIF
head), then

```
/usr/bin/time -f "%e s wall %U s user %M kB" ./wba border <dir> --apply --force --workers 14
```

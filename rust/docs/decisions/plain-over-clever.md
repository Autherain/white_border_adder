# Plain control flow over combinator chains

Rust lets a function be one expression of `.and_then().map_or_else()`.
This code does not do that: the reader comes from Go and wants to see the
branches.

Rules applied to the production code:

- `match`, `if let`, `let ... else` and `for` instead of `.and_then`,
  `.or_else`, `.map_or`, `.filter_map`, `.fold`, `.zip`, `.then_some`,
  `.is_some_and`.
- An iterator chain stops at one step: `.iter().map(f).collect()` or
  `.iter().filter(p).count()` are fine, two closures in a row are not.
- Closures are for the thing that is a callback (`run_each`, `sort_by`,
  `dated(name)`), never a way to skip naming a variable.
- Tests use `.unwrap()` where Go would `t.Fatal(err)`.

The one idiom to learn is `?`:

```rust
let head = read_head(path)?;        // Option: return None if read_head did
let entries = dir.read_dir()?;      // Result: return Err(e.into()) if it failed
```

It is Go's `if err != nil { return err }` on one character, and it also
works on `Option`, where Go would write `if !ok { return x, false }`.

Three conversions come back often:

| Rust | Go |
|---|---|
| `let Ok(x) = f() else { return None };` | `x, err := f(); if err != nil { return 0, false }` |
| `let Some(x) = opt else { continue };` | `x, ok := m[k]; if !ok { continue }` |
| `match f() { Ok(x) => x, Err(e) => return Err(e.into()) }` | `x, err := f(); if err != nil { return fmt.Errorf("...: %w", err) }` |

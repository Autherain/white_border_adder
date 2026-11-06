# Plain control flow over combinator chains

Rust lets a function be one expression of `.and_then().map_or_else()`.
This code does not do that: the reader wants to see the branches.

Rules applied to the production code:

- `match`, `if let`, `let ... else` and `for` instead of `.and_then`,
  `.or_else`, `.map_or`, `.filter_map`, `.fold`, `.zip`, `.then_some`,
  `.is_some_and`.
- An iterator chain stops at one step: `.iter().map(f).collect()` or
  `.iter().filter(p).count()` are fine, two closures in a row are not.
- Closures are for the thing that is a callback (`run_each`, `sort_by`,
  `dated(name)`), never a way to skip naming a variable.
- Tests use `.unwrap()`: a fixture that fails to build is a broken test,
  not a case to handle.

The one idiom to lean on is `?`:

```rust
let head = read_head(path)?;        // Option: return None if read_head did
let entries = dir.read_dir()?;      // Result: return Err(e.into()) if it failed
```

Three shapes come back often:

| Shape | Meaning |
|---|---|
| `let Ok(x) = f() else { return None };` | the error is not worth reporting here, the caller sees "nothing" |
| `let Some(x) = opt else { continue };` | skip this item and go on |
| `match f() { Ok(x) => x, Err(e) => return Err(e.into()) }` | the error is wrapped in this module's own error type |

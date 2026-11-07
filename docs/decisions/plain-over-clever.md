# Plain control flow over combinator chains

Rust lets a function be one expression of `.and_then().map_or_else()`.
This code does not do that: the reader wants to see the branches.

Rules applied to the production code:

- `match`, `if let` and `for` instead of `.and_then`, `.or_else`,
  `.map_or`, `.filter_map`, `.fold`, `.zip`, `.then_some`, `.is_some_and`,
  `.then`.
- No `let ... else`, `.unwrap_or(x)`, `.unwrap_or_default()`, `.ok()`,
  `.map_err(f)?` or `iter::once(a).chain(b)`. A two-arm `match` (or an
  `if let Err(e)`) says the same thing with both branches on their own
  line, the way `v, ok := f(); if !ok { return }` does in Go, which is
  what the owner reads. The clippy lints that would push these back
  (`manual_let_else`, `manual_unwrap_or`, `manual_unwrap_or_default`,
  `manual_ok_err`) are allowed in `Cargo.toml` for that reason.
- `.unwrap()` never appears outside tests; clippy's `unwrap_used` is on.
  A value that cannot be absent is still taken out with a `match` whose
  other arm says what happens.
- An iterator chain stops at one step: `.iter().map(f).collect()` or
  `.iter().filter(p).count()` are fine, two closures in a row are not.
- Closures are for the thing that is a callback (`run_each`, `sort_by`,
  `dated(name)`), never a way to skip naming a variable.
- Tests use `.unwrap()`: a fixture that fails to build is a broken test,
  not a case to handle.

Three rules decide how a fallible call is written:

1. `?` when the error goes up unchanged. It is Go's
   `v, err := f(); if err != nil { return err }` in one character, and the
   compiler refuses the call if the `?` is missing.

   ```rust
   let abs = std::path::absolute(arg)?;
   ```

2. A two-arm `match` when the failing branch does something: wraps the
   error in this module's variant, picks a fallback, skips the item or
   returns `None`. Both branches sit on their own line.

3. Never `is_err()` or `is_none()` followed by `unwrap()`. The test and
   the extraction would be two statements, and nothing stops a later edit
   from removing one without the other. A `match` does both at once.

A function whose last statement is `if c { a } else { b }` returns early
instead: `if c { return a; } b`. The `let x = if c { a } else { b };`
form stays: it is the ternary Go does not have.

The shapes that come back often:

| Shape | Meaning |
|---|---|
| `match f() { Ok(x) => x, Err(_) => return None }` | the error is not worth reporting here, the caller sees "nothing" |
| `match opt { Some(x) => x, None => continue }` | skip this item and go on |
| `match f() { Ok(x) => x, Err(e) => return Err(Error::Read(e)) }` | the error is wrapped in this module's own error type |
| `if let Err(e) = f() { return Err(Error::Mkdir(e)) }` | same, for a call whose success value is `()` |
| `match f() { Ok(x) => x, Err(_) => fallback }` | a default stands in for the failure |

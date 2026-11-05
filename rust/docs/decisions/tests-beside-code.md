# Tests in `<file>_test.rs`, next to `<file>.rs`

Rust's default puts tests in a `#[cfg(test)] mod tests { ... }` block at the
bottom of the file. For a 400-line table-driven suite that buries the code.

```rust
#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;
```

`#[path]` tells the compiler where the module's file is; without it,
`mod tests;` inside `plan.rs` would look for `plan/tests.rs`. The test file
is still a child module of `plan`, so `use super::*;` reaches private
functions, and it only compiles under `cargo test`.

A test file that other test files borrow from (`show_test.rs` lends
`test_app`, `exif_test.rs` lends `tiff_with`) is declared `pub(crate) mod
tests;` and the borrower writes `use crate::cli::show::tests::test_app;`.

The package root (`clean/mod.rs`) is tested from `mod_test.rs`; a flat
module (`glob.rs`) from `glob_test.rs`.

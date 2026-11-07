# A trait in a struct field is `Box<dyn Trait>`, not a type parameter

`App` holds its logger as `log: Box<dyn Log>`. The first port had
`struct App<L: Log> { log: L }` and `impl<L: Log> App<L>` on four files.
Nothing needed that; it was the Rust reflex, not a decision.

## The two forms, side by side with Go

```go
type App struct{ log Log }          // interface in a field: what Go code writes
type App[L Log] struct{ log L }     // generic struct: never for a behaviour
```

```rust
struct App { log: Box<dyn Log> }    // same as the Go interface field
struct App<L: Log> { log: L }       // same as the Go generic struct
```

The generic form repeats `<L>` on every `impl`, every test helper's return
type and every place that names `App`. The trait-object form keeps `App`
one plain type. The `Box` is only there because a field must have a known
size and `dyn Log` has none; Go hides the same two pointers inside its
interface value.

Rule kept from Go: a type parameter is for a data type (`T` in a `[]T`,
`Vec<T>`), an interface is for a behaviour (`Log`, `Shape`). The `plan`
functions already took `&dyn Log`; now the whole crate uses the trait the
one way.

## What each form costs, and why it does not matter here

Calling a method through an interface is an indirect jump: the CPU reads
the function's address from a small table attached to the value, then
jumps there. A Go interface call and a Rust `dyn` call both do that. With
a generic, the compiler emits one copy of the function per concrete type
(`total_circle`, `total_square`), each calling its method at a fixed
address; the trait only checked the code at compile time and is gone from
the binary. That lets the compiler paste the method's body into the loop,
and a loop with no call in it can be vectorised. Go's generics do not do
this; they also go through a table.

An indirect jump is about a nanosecond. The heap has nothing to do with
it: reading a value from the heap is as fast as reading it anywhere else.

So a generic pays only in a hot loop, millions of calls per run: a
per-pixel filter, a hash in a table, a comparison in a sort. A logger that
writes a hundred lines per run is never that. `render.rs` does its pixel
work directly, with no trait in the loop, so this crate has no place where
`<T: Trait>` would buy anything.

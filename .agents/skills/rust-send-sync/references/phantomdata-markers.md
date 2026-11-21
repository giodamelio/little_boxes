# `PhantomData` markers for `Send` and `Sync`

`PhantomData<X>` inherits `X`'s auto traits exactly, so any std type with the wanted
implementation works as a marker. Pick the marker that removes only what you mean to remove:

| Marker field | `Send` | `Sync` |
| --- | --- | --- |
| `PhantomData<fn() -> T>` | kept | kept |
| `PhantomData<Cell<()>>` | kept | removed |
| `PhantomData<MutexGuard<'static, ()>>` | removed | kept |
| `PhantomData<*const ()>` | removed | removed |

Write `()` in the last three markers. With `T` in its place, "kept" becomes conditional:
`Cell<T>` is `Send` only when `T: Send`, and `MutexGuard<'_, T>` is `Sync` only when `T: Sync`.

A marker that names your own `T` also sets the variance in `T` and the drop-check ownership. The
`rust-variance` skill, when it is installed, owns the table for both; `PhantomData<fn() -> T>`
stays covariant without ownership, and `PhantomData<fn(T) -> T>` is invariant and keeps `Send`
and `Sync` for every `T`.

```rust
use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::MutexGuard;

struct SendNotSync(PhantomData<Cell<()>>);                 // moves threads, does not share
struct SyncNotSend(PhantomData<MutexGuard<'static, ()>>);  // shares, stays on its thread
struct NeitherOne(PhantomData<*const ()>);                 // both removed
struct BothKept<T>(PhantomData<fn() -> T>);                // both kept for every T

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn main() {
    assert_send::<SendNotSync>();
    assert_sync::<SyncNotSend>();
    assert_send::<BothKept<std::rc::Rc<i32>>>();
    assert_sync::<BothKept<std::rc::Rc<i32>>>();
    let _ = NeitherOne(PhantomData);   // constructs, but neither Send nor Sync
}
```

Rules:

- Reach for `PhantomData<*mut ()>` only when you mean "neither". It is the default reflex and it
  is wrong when you only meant "not shareable": it also blocks moving the value to a worker
  thread, and the diagnostic then names `*mut ()`, a type the reader cannot find in the struct.
- Write `()` inside the marker when the marker only drops an auto trait. A marker that names your
  own parameter also constrains variance: `PhantomData<Cell<T>>` makes the struct invariant in
  `T`, which rejects caller substitutions that look safe.
- `PhantomData<MutexGuard<'static, T>>` goes further than the other four markers. `MutexGuard<'a,
  T>` is declared `T: ?Sized + 'a`, so a naked `T` in this marker forces `T: 'static` on the
  struct, and rustc rejects the definition with `error[E0310]`. Write `MutexGuard<'static, ()>`
  for the auto trait effect, and name `T` in a second marker.

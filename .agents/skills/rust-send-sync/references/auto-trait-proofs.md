# Auto trait proofs

Deep material for `rust-send-sync`. `SKILL.md` holds each rule; this file holds the programs that
prove it. Each block compiles, fails with the named code, or runs, on rustc 1.99.1, edition 2024.
The full diagnostics are in `rustc-transcripts.md`.

Contents:

- [Probe helpers](#probe-helpers)
- [`&Cell<i32>` is not `Send`](#cell-is-not-send-by-reference)
- [`Arc` needs both traits on the payload](#arc-needs-both-traits-on-the-payload)
- [`&mut T` needs `T: Send`](#mut-t-needs-t-send)
- [`Mutex` and `RwLock` payloads](#mutex-and-rwlock-payloads)
- [Guards](#guards)
- [`+ Send` on an `impl Trait` return](#-send-on-an-impl-trait-return)

## Probe helpers

Two helper functions turn any question into a compile error you can read:

```rust
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn main() {
    assert_send::<&i32>();                  // i32 is Sync, so &i32 is Send
    assert_sync::<&i32>();                  // &T is Sync exactly when T is Sync
    assert_send::<std::cell::Cell<i32>>();  // Cell is Send: a single owner moves it
}
```

## `&Cell` is not `Send` by reference

`Cell<i32>` is `Send`. A shared reference to it is not:

```rust,compile_fail,E0277
use std::cell::Cell;
fn assert_send<T: Send>() {}
fn main() { assert_send::<&Cell<i32>>(); }
```

```text
error[E0277]: `&Cell<i32>` cannot be sent between threads safely
3 | fn main() { assert_send::<&Cell<i32>>(); }
  |                           ^^^^^^^^^^ `&Cell<i32>` cannot be sent between threads safely
  |
  = help: the trait `Sync` is not implemented for `Cell<i32>`
  = note: required for `&Cell<i32>` to implement `Send`
```

The head trait is `Send`, because that is the bound that failed. The unmet obligation is `Sync`,
on the pointee. The failing impl is `impl<T: Sync + ?Sized> Send for &T`.

## `Arc` needs both traits on the payload

```rust,compile_fail,E0277
use std::cell::Cell;
use std::sync::Arc;
fn assert_send<T: Send>() {}
fn main() {
    assert_send::<Cell<i32>>();        // ok on its own
    assert_send::<Arc<Cell<i32>>>();   // E0277: Arc<T>: Send needs T: Send + Sync
}
```

The error says `` `Cell<i32>` cannot be shared between threads safely ``, with a `note:` that
suggests `RwLock` or `AtomicI32`. The note means "replace the `Cell`".

## `&mut T` needs `T: Send`

```rust,compile_fail,E0277
use std::cell::Cell;
use std::rc::Rc;
fn assert_send<T: Send>() {}
fn main() {
    assert_send::<&mut Cell<i32>>();  // ok: Cell<i32> is Send, although it is not Sync
    assert_send::<&mut Rc<i32>>();    // E0277: &mut T: Send needs T: Send
}
```

A `&mut T` is a move channel, not a view. `std::mem::replace(x, T::default())` takes the value out
through the reference, so the receiving thread owns and drops it.

## `Mutex` and `RwLock` payloads

A `Mutex` hands out one `&mut T` at a time, so the payload never has to be shareable. An `RwLock`
hands out many concurrent `&T`, so the payload must be `Sync` on its own.

```rust
use std::cell::Cell;
use std::sync::Mutex;
fn assert_sync<T: Sync>() {}
fn main() { assert_sync::<Mutex<Cell<i32>>>(); }   // compiles
```

```rust,compile_fail,E0277
use std::cell::Cell;
use std::sync::RwLock;
fn assert_sync<T: Sync>() {}
fn main() { assert_sync::<RwLock<Cell<i32>>>(); }
```

## Guards

POSIX requires the unlocking thread to be the locking thread, so the guard's `Drop` must run where
the guard was created:

```rust,compile_fail,E0277
use std::sync::MutexGuard;
fn assert_send<T: Send>() {}
fn main() { assert_send::<MutexGuard<'static, i32>>(); }
```

The guard is `Sync` whenever `T: Sync`, so `&MutexGuard<'_, T>` is `Send`. This runs and prints
`total=6`:

```rust,run
use std::sync::Mutex;
use std::thread;

fn main() {
    let lock = Mutex::new(vec![1u32, 2, 3]);
    let guard = lock.lock().expect("lock poisoned");
    let total: u32 = thread::scope(|s| {
        // &MutexGuard is Send, because MutexGuard is Sync.
        s.spawn(|| guard.iter().sum::<u32>()).join().unwrap()
    });
    assert_eq!(total, 6);
    println!("total={total}");
    drop(guard);   // the unlock runs on the locking thread
}
```

## `+ Send` on an `impl Trait` return

```rust
pub fn ids() -> impl Iterator<Item = u32> + Send {
    (0..3).map(|i| i * 2)
}
fn main() { assert_eq!(ids().sum::<u32>(), 6); }
```

# rustc transcripts for `Send` and `Sync` errors

Every transcript comes from rustc 1.99.1, edition 2024, on x86_64-unknown-linux-gnu. The quotes
are excerpts. `SKILL.md` holds the rule and the fix for each case; this file holds the full
diagnostics and the longer examples.

Contents:

- [`Arc<Cell<i32>>`](#arccelli32-is-not-send)
- [`&mut Rc<i32>`](#mut-rci32-is-not-send)
- [E0321 on a reference type](#e0321-on-a-reference-type)
- [`RwLock<Cell<i32>>`](#rwlockcelli32-is-not-sync)
- [`MutexGuard`](#mutexguard-is-not-send)
- [Auto trait leak from `impl Trait`](#auto-trait-leak-from-impl-trait)
- [The async diagnostic](#the-async-diagnostic)

## `Arc<Cell<i32>>` is not `Send`

```text
error[E0277]: `Cell<i32>` cannot be shared between threads safely
  = help: the trait `Sync` is not implemented for `Cell<i32>`
  = note: if you want to do aliasing and mutation between multiple threads, use
          `std::sync::RwLock` or `std::sync::atomic::AtomicI32` instead
  = note: required for `Arc<Cell<i32>>` to implement `Send`
```

The `note:` means "replace the `Cell`". Do not wrap the `Cell` in the suggested `RwLock`.

## `&mut Rc<i32>` is not `Send`

```text
error[E0277]: `Rc<i32>` cannot be sent between threads safely
  = help: within `&mut Rc<i32>`, the trait `Send` is not implemented for `Rc<i32>`
  = note: required because it appears within the type `&mut Rc<i32>`
```

## E0321 on a reference type

The impl that decides per reference type whether it is `Send` cannot be written:

```rust,compile_fail,E0321
pub struct Handle(*mut u8);
unsafe impl Send for &Handle {}
unsafe impl Send for &mut Handle {}
```

```text
error[E0321]: cross-crate traits with a default impl, like `Send`, can only be implemented
              for a struct/enum type, not `&Handle`
2 | unsafe impl Send for &Handle {}
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ can't implement cross-crate trait with a default impl
  |                              for non-struct/enum type
```

rustc reports one E0321 per impl, so this block gives a second error for `&mut Handle`.

## `RwLock<Cell<i32>>` is not `Sync`

```text
error[E0277]: `Cell<i32>` cannot be shared between threads safely
  = help: the trait `Sync` is not implemented for `Cell<i32>`
  = note: required for `std::sync::RwLock<Cell<i32>>` to implement `Sync`
```

## `MutexGuard` is not `Send`

```text
error[E0277]: `std::sync::MutexGuard<'static, i32>` cannot be sent between threads safely
  = help: the trait `Send` is not implemented for `std::sync::MutexGuard<'static, i32>`
```

## Auto trait leak from `impl Trait`

A public `-> impl Iterator<Item = u32>` whose body captures an `Rc` compiles. The error appears
at a caller that sends the iterator to a thread, with a chain of `note:` lines that ends in the
private body:

```text
error[E0277]: `Rc<Vec<u32>>` cannot be sent between threads safely
10 |     std::thread::spawn(move || { let _: Vec<u32> = it.collect(); });
   |                        ^^^^^^^ within this `{closure@...}`
note: required because it appears within the type `impl Iterator<Item = u32>`
```

With `+ Send` written in the signature, the same body fails at the definition:

```rust,compile_fail,E0277
use std::rc::Rc;
pub fn ids() -> impl Iterator<Item = u32> + Send {
    let names: Rc<Vec<u32>> = Rc::new(vec![1, 2, 3]);
    (0..3).map(move |i| names[i])
}
```

```text
error[E0277]: `Rc<Vec<u32>>` cannot be sent between threads safely
3 | pub fn ids() -> impl Iterator<Item = u32> + Send {
  |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rc<Vec<u32>>` cannot be sent ...
5 |     (0..3).map(move |i| names[i])
  |     ----------------------------- return type was inferred to be `Map<...>` here
```

The desugared form of an `async fn` that freezes `Send` in the signature:

```rust
use std::future::Future;

pub fn load(id: u32) -> impl Future<Output = u32> + Send {
    async move { id + 1 }
}

fn main() { let _ = load(1); }
```

## The async diagnostic

The message has no error code, so a search for E0277 finds nothing:

```text
error: future cannot be sent between threads safely
  |                         ^^^^ future created by async block is not `Send`
note: future is not `Send` as this value is used across an await
5 |         let names = Rc::new(vec![1u32, 2, 3]);
  |             ----- has type `Rc<Vec<u32>>` which is not `Send`
6 |         std::future::ready(()).await;
  |                                ^^^^^ await occurs here, with `names` maybe used later
```

The `note:` names the value and the exact `.await` that traps it.

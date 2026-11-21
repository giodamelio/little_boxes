---
description: Use when deciding whether a type is Send or Sync, or when rustc rejects a value at a thread or task boundary with "cannot be sent between threads safely", "cannot be shared between threads safely", or "future cannot be sent between threads safely". Also for auto trait rules of references and smart pointers (Arc vs Rc), Mutex vs RwLock payload bounds, why MutexGuard is not Send yet is Sync, PhantomData markers that remove Send or Sync, auto trait leakage from impl Trait and async fn, and E0321. Not for proving a manual unsafe impl Send or Sync; use `rust-unsafe`.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-send-sync
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 1266e82ab6f856170b720cbefb5b639483ef83a5
name: rust-send-sync
---
# Rust Send and Sync

Every error text below comes from rustc 1.99.1, edition 2024, on x86_64-unknown-linux-gnu. The
quotes are excerpts. Read [references/rustc-transcripts.md](references/rustc-transcripts.md) when
an error does not match a short quote here, and
[references/auto-trait-proofs.md](references/auto-trait-proofs.md) for a program that proves a
rule below.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| `error[E0277]: X cannot be sent between threads safely` with `help: the trait Sync is not implemented` | [The one rule](#the-one-rule-t-is-send-exactly-when-t-is-sync) |
| `error[E0277]: X cannot be shared between threads safely` | [The auto trait table](#the-auto-trait-table) |
| `error[E0321]: cross-crate traits with a default impl, like Send` | [E0321](#unsafe-impl-send-cannot-target-a-reference-e0321) |
| A `Mutex` was changed to an `RwLock` and the build broke | [Lock payload bounds](#lock-payload-bounds-mutex-and-rwlock-are-not-interchangeable) |
| `MutexGuard<'_, T> cannot be sent between threads safely` | [Guards](#guards-mutexguard-is-not-send-and-it-is-sync) |
| You want a type that moves between threads but is not shareable | [`PhantomData` surgery](#phantomdata-surgery) |
| An unrelated call site broke after you edited a function body | [Auto traits leak](#auto-traits-leak-out-of-impl-trait-and-async-fn) |
| `error: future cannot be sent between threads safely` | [Auto traits leak](#auto-traits-leak-out-of-impl-trait-and-async-fn) |
| ``warning: use of `async fn` in public traits is discouraged`` | the `rust-async-internals` skill, when it is installed |
| A public type must keep or lose `Send` or `Sync` on purpose | [Verify](#verify-the-auto-traits) |
| You are about to write `unsafe impl Send for MyType {}` | the `rust-unsafe` skill, when it is installed |

## The one rule: `&T` is `Send` exactly when `T` is `Sync`

`Sync` states that one fact at the type level. A `Send` error whose `help:` line names `Sync`
(``the trait `Sync` is not implemented for `Cell<i32>` `` under ``note: required for `&Cell<i32>`
to implement `Send` ``) therefore does not ask for a `Send` impl. It reports a shared reference
to a value that is not shareable. Both auto traits are derived field-wise. Fix a bound error in
the fields, not with an impl. Probe a question with `fn assert_send<T: Send>() {}` and
`assert_send::<&Cell<i32>>();`.

Repair in this order. Delete the sharing and move the value instead of the reference. Or give the
pointee interior mutability that is `Sync`: an atomic, a `Mutex`, or an `RwLock`. Only then read
the `rust-unsafe` skill, and only for a type you own whose own invariant makes the impl sound.

Never add `unsafe impl Send` or `unsafe impl Sync` only to make an error in this skill go away,
including on a wrapper struct around an `Rc`, a `Cell`, or a guard. The compiler accepts the impl
whether or not it is sound. Fix the field, the lock, or the signature instead.

`cannot be sent` is a failed `Send` bound; `cannot be shared` is a failed `Sync` bound. The second
appears under an `Arc`, because `Arc<T>: Send` itself requires `T: Sync`.

## Verify the auto traits

The compiler proves only the auto traits it derives from fields. After a manual `unsafe impl`, it
no longer checks what a raw pointer field points to, so a green build proves nothing about that
impl.

| Claim | Check |
| --- | --- |
| A public type stays `Send` or `Sync` | A `const` assertion next to the type (below). A field edit then fails at the definition, not at a distant caller. Across releases, `cargo semver-checks` reports `auto_trait_impl_removed` |
| A public `-> impl Trait` return stays `Send` | Write `+ Send` (and `+ Sync` when callers share it) in the signature. See [Auto traits leak](#auto-traits-leak-out-of-impl-trait-and-async-fn) |
| A type is deliberately not `Send` | A `compile_fail` doctest that defines `assert_send` and calls `assert_send::<my_crate::Type>()`, and nothing else: it passes on any error, and stable rustdoc does not check an error code. A doctest is a separate crate, so `crate::Type` fails for the wrong reason |
| An `Arc` payload is shareable | `clippy::arc_with_non_send_sync`, warn by default |
| No `std::sync` guard lives across an `.await` | `clippy::await_holding_lock`, warn by default |
| Every `async fn` future is `Send` | `clippy::future_not_send` (nursery). Set `future_not_send = "warn"` under `[lints.clippy]` in a library whose futures cross `spawn`, and put `#[expect(clippy::future_not_send, reason = "...")]` on each future that is local by design. It proves nothing about a generic `async fn` |

Run `cargo clippy --locked --all-targets -- -D warnings` before a commit that touches a type or a
future that crosses threads.

```rust
pub struct Session { pub id: u32, pub buf: Vec<u8> }

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Session>();
};
```

## The auto trait table

| Type | `Send` when | `Sync` when |
| --- | --- | --- |
| `T` (your struct) | every field is `Send` | every field is `Sync` |
| `&T` | `T: Sync` | `T: Sync` |
| `&mut T` | `T: Send` | `T: Sync` |
| `Box<T>` | `T: Send` | `T: Sync` |
| `Arc<T>` | `T: Send + Sync` | `T: Send + Sync` |
| `Rc<T>` | never | never |
| `*const T`, `*mut T` | never | never |
| `Cell<T>`, `RefCell<T>` | `T: Send` | never |
| `Mutex<T>`, `RwLock<T>` | `T: Send` | see [lock bounds](#lock-payload-bounds-mutex-and-rwlock-are-not-interchangeable) |
| `MutexGuard<'_, T>`, `RwLockReadGuard`, `RwLockWriteGuard` | never | `T: Sync` |

Two rows carry the surprise:

- **`Arc<T>` needs both traits on the payload.** An `Arc` clone hands a second owner shared
  access, so `Arc<Cell<i32>>` fails with `` `Cell<i32>` cannot be shared between threads safely ``
  and a `note:` that suggests `RwLock` or `AtomicI32`. Replace the `Cell`: `Arc<AtomicI32>`,
  `Arc<RwLock<i32>>`, and `Arc<Mutex<Cell<i32>>>` all work. Do not wrap the `Cell` in the
  suggested `RwLock`: `RwLock<Cell<i32>>` fails again. `clippy::arc_with_non_send_sync` reports
  such an `Arc::new` at the construction site; if that `Arc` never crosses a thread, use `Rc`.
- **`&mut T` is not the mirror of `&T`.** `&mut T: Send` requires `T: Send`, not `T: Sync`, because
  `std::mem::replace` moves the value out through it. `&mut Cell<i32>` is `Send`;
  `&mut Rc<i32>` is not.

## `unsafe impl Send` cannot target a reference (E0321)

`unsafe impl Send for &Handle {}` fails with `error[E0321]: cross-crate traits with a default impl,
like Send, can only be implemented for a struct/enum type`. `Sync` is the only lever that reaches
`&Handle`: `unsafe impl Sync for Handle {}` makes `&Handle` `Send` through the blanket impl. That
is a stronger promise, and the `rust-unsafe` skill holds the audit it needs.

## Lock payload bounds: `Mutex` and `RwLock` are not interchangeable

- `Mutex<T>: Sync` requires only `T: Send`.
- `RwLock<T>: Sync` requires `T: Send + Sync`.

Changing a read-heavy `Mutex<T>` to an `RwLock<T>` is therefore **not** a drop-in change. It stops
compiling as soon as the payload holds a `Cell` or a `RefCell`, and the error lands on the
`Arc<RwLock<...>>` at a call site far from the edit. Check the payload first. An `Rc` payload is
not a symptom of the swap: `Mutex<Rc<T>>` is neither `Send` nor `Sync` before the edit. The
`Send` direction gives no warning, because both locks are `Send` when `T: Send`.

## Guards: `MutexGuard` is not `Send`, and it is `Sync`

`std::sync::MutexGuard<'_, T>` is `!Send` for every `T`, because the unlock must run on the
locking thread. The guard is `Sync` whenever `T: Sync`, so `&MutexGuard<'_, T>` **is** `Send`.

- Give a worker `&guard` only when the payload is `Sync`: `&MutexGuard<'_, Cell<T>>` does not
  cross. Clone the value out of the guard instead. Never give a worker the guard itself.
- Pass `&guard` through `thread::scope`. `thread::spawn` requires `'static`, so it rejects the
  borrow with ``error[E0597]: `lock` does not live long enough``, plus `error[E0373]` when the
  closure borrows `guard` directly. Do not apply the E0373 `move` suggestion: it moves the guard
  itself, and that fails with E0277.
- Do not hold a `std::sync` guard across an `.await`. The task can resume on another thread, so
  the future stops being `Send`. `clippy::await_holding_lock` catches it.

## `PhantomData` surgery

`PhantomData<X>` inherits the auto traits of `X`. Use `PhantomData<Cell<()>>` to remove only
`Sync`. Use a raw pointer marker such as `PhantomData<*mut ()>` only when you mean to remove both,
because it also blocks a move to a worker thread. Write `()` in the marker, not your own `T`: a
marker that names `T` also changes variance. Read
[references/phantomdata-markers.md](references/phantomdata-markers.md) when you design a marker:
it has the auto-trait marker table, and the marker that removes only `Send`.

## Auto traits leak out of `impl Trait` and `async fn`

A public `-> impl Trait` with no explicit `+ Send` publishes whatever auto traits the **body**
happens to have. Adding one `Rc` inside the function is then a breaking change, reported at every
distant call site with a `note: required because it appears within the type impl Iterator<Item =
u32>` that points into your private body. Spell the bound in the signature
(`fn ids() -> impl Iterator<Item = u32> + Send`): the guarantee is frozen, and the same body
fails at the definition. Write `+ Send` only when you intend it, and add `+ Sync` when callers
share the value. Record a single-threaded-by-design decision in the doc comment.

`async fn f() -> T` desugars to `fn f() -> impl Future<Output = T>` and has the same leak. On
stable Rust 1.99.1, no syntax bounds an `async fn` return type, so write the desugared form:
`fn load(id: u32) -> impl Future<Output = u32> + Send { async move { id + 1 } }`. Do not silence
the `async_fn_in_trait` warning when callers spawn the future; the `rust-async-internals` skill,
when it is installed, covers `async fn` in a trait.

The async diagnostic, `error: future cannot be sent between threads safely`, has **no error
code**, so a search for E0277 finds nothing. Its `note: future is not Send as this value is used
across an await` names the value and the `.await` that traps it. Read
[references/rustc-transcripts.md](references/rustc-transcripts.md) for both transcripts.

## Related skills

When installed: `rust-unsafe` (the audit behind a manual `unsafe impl Send` or `Sync`),
`rust-variance` (the variance a `PhantomData` marker adds), `rust-compiler-errors` (reading E0277
with `rustc --explain E0277`), `rust-async-internals` (async locks and `Send` trait variants),
`memory-model` (atomics and orderings), and `rust-crate-release` (semver review of an auto trait
change with `cargo semver-checks`).

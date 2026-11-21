---
description: Use when a lifetime coercion is refused ("is invariant over the parameter", "borrowed for 'static", "lifetime may not live long enough" on a return or argument), when deciding whether a type is covariant, contravariant, or invariant, or when adding a lifetime parameter, a PhantomData variance marker, or interior mutability to a public type. Also for subtyping of a fn item that fails a trait bound (resize_with), dyn Fn lifetime parameters, and an unbounded lifetime from a raw pointer. Triggers on "phantomdata variance", "&mut is invariant", "sender is invariant".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-variance
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: dd1e69de03ce4d08645efe2db69eee3cf8921f13
name: rust-variance
---
# Rust variance

Does a value that holds `'b` fit where the compiler asks for `'a`, given `'b: 'a`? The type
constructor around the lifetime decides. A trait bound matches by equality, so nothing coerces
through it.

Every quoted diagnostic comes from rustc 1.98.1, edition 2024, aarch64-apple-darwin.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| The variance of a type you own; `mutable references are invariant`; an invariant type that accepts a shorter lifetime through `dyn` or an unsize | [Settle it with a probe](#settle-it-with-a-probe) |
| `note: the struct X<T> is invariant over the parameter T` | [The variance table](#the-variance-table) |
| A raw-pointer deref returns `&'a T` with no `'a` in the inputs, or `raw pointer casts of trait objects cannot extend lifetimes` | [Unbounded lifetimes](#unbounded-lifetimes-come-from-a-raw-pointer) |
| A `Sender`, handle, or queue refuses to coerce | [Coerce the message](#coerce-the-message-not-the-handle) |
| You add `Cell`, `RefCell`, or `Mutex` to a published struct | [Variance is public API](#variance-is-public-api) |
| E0597 at `resize_with`, `map`, or a callback argument, with the `'static` note on a trait bound | [Traits match by equality](#traits-match-their-parameters-by-equality) |
| `error[E0106]: missing lifetime specifier` on `fn() -> &str` | [Three fixes](#three-fixes-for-a-producer-whose-output-lives-too-long) |
| `lifetime may not live long enough` on a `Box<dyn Fn(&'a T)>` field | [A free lifetime in `dyn Trait`](#a-free-lifetime-in-dyn-trait-makes-it-invariant) |
| `type annotation requires that ... is borrowed for 'static` at a `&mut` argument | [A `&mut` parameter](#a-mut-parameter-pins-the-callers-lifetime) |
| Scripted probes, every exact note, a rejected and repaired pair, or the unsound channel | Read [references/variance-tables.md](references/variance-tables.md) |

## Settle it with a probe

Do not reason about variance. Compile these three functions and read the result.

```rust,ignore
// probe.rs — replace `C` with the type under test, `S` with a type that has a lifetime.
fn cov<'a, 'b: 'a>(x: C<&'b u8>) -> C<&'a u8> { x }   // compiles => covariant in T
fn con<'a, 'b: 'a>(x: C<&'a u8>) -> C<&'b u8> { x }   // compiles => contravariant in T
fn cov_lt<'a, 'b: 'a>(x: S<'b>) -> S<'a> { x }        // compiles => covariant in 'a
```

Run one command with a real output path. Never pass `-o /dev/null`: as root, rustc replaces the
device with a regular file and exits 0.

```bash
rustc --edition 2024 --crate-type lib --emit=metadata probe.rs -o probe.rmeta
```

| `cov` | `con` | Verdict |
| --- | --- | --- |
| compiles | rejected | covariant in `T` |
| rejected | compiles | contravariant in `T` |
| rejected | rejected | invariant in `T` |

`Vec<T>` passes `cov`. `fn(T)` passes `con`. `Cell<T>` fails both, and rustc names the rule:
``the struct `Cell<T>` is invariant over the parameter `T` ``. Read
[references/worked-examples.md](references/worked-examples.md) when you need a cited program or
diagnostic in full.

Two traps make a probe lie. Never put `'static` in the outer position: `&'static mut &'b u8`
forces `'b: 'static`; use a fresh `'x`. Never let a probe unsize-coerce: a std pointer or cell
that holds `dyn Trait + 'b` shortens even when invariant, so wrap it in `Vec<...>`. Read
[references/variance-tables.md](references/variance-tables.md#probe-traps) for both traps and the
per-version unsize table.

## The variance table

| Constructor | Variance in `T` | Variance in `'a` |
| --- | --- | --- |
| `&'a T` | covariant | covariant |
| `&'a mut T` | **invariant** | covariant |
| `*const T`, `fn() -> T`, `Box<T>`, `Vec<T>`, `Rc<T>`, `Arc<T>`, `Option<T>`, `[T; N]`, `(T, U)` | covariant | — |
| `*mut T` | **invariant** | — |
| `fn(T)` | **contravariant** | — |
| `fn(T) -> T` | **invariant** | — |
| `Cell<T>`, `UnsafeCell<T>`, `RefCell<T>`, `Mutex<T>`, `RwLock<T>`, `mpsc::Sender<T>` | **invariant** | — |
| `dyn Trait + 'a` | — | covariant |
| `dyn Fn(&'a T)`, `dyn Fn() -> &'a T` with a free `'a` | — | **invariant** |
| `PhantomData<T>`, `PhantomData<&'a T>`, `PhantomData<fn() -> T>` | covariant | covariant |
| `PhantomData<fn(T)>` | **contravariant** | — |
| `PhantomData<*mut T>`, `PhantomData<Cell<T>>`, `PhantomData<&'a mut T>` | **invariant** | covariant |

Two rules generate the whole table:

- A struct or enum computes variance per parameter from its fields. Uses of different variances
  make the parameter **invariant**: one `Cell<T>` field, or a `fn() -> T` field next to a
  `fn(T)` field.
- Everything built on `UnsafeCell<T>` is invariant in `T`, because a shared reference to it
  permits a write: `Cell`, `RefCell`, `Mutex`, `RwLock`, `AtomicPtr`, and every `std` channel.

A parameter that no field stores needs a `PhantomData` that states the intent:
`PhantomData<&'a T>` to borrow, `PhantomData<T>` to own, `PhantomData<fn(T)>` for
contravariance, `PhantomData<*mut T>` or `PhantomData<Cell<T>>` for invariance.
`PhantomData<fn(T)>` is sound only for a type that never stores or drops a `T`.
`PhantomData<fn(T) -> T>` gives invariance and keeps `Send` and `Sync`; `PhantomData<*mut T>`
removes both. Read the `PhantomData` table in [references/variance-tables.md](references/variance-tables.md)
when the auto traits matter.

## Unbounded lifetimes come from a raw pointer

An output lifetime that appears in no input is unbounded: every call site picks its own. A
raw-pointer deref is the usual source: `unsafe fn deref_unbounded<'a, T>(p: *const T) -> &'a T`
compiles, lets the result outlive its owner, and Miri reports a use-after-free. So do
`<*const T>::as_ref`, `<*mut T>::as_mut`, `NonNull::as_ref` and `as_mut`, `as_ref_unchecked` and
`as_mut_unchecked` (stable since 1.95), `slice::from_raw_parts`, and `CStr::from_ptr`.

Borrowing the pointer variable proves that the pointer is alive, not its pointee. Tie the output
to the actual owner (`&self` on a wrapper that owns the pointer), or return a guard that keeps
the allocation alive. An `unsafe` constructor must state and enforce the pointee-validity
contract. Run `cargo +nightly miri test --locked` on every path that turns a raw pointer into a
reference when nightly Miri is installed; without it, report that path as not checked by Miri.

Since Rust 1.94 a raw-pointer cast cannot extend the lifetime bound of a trait object
(`note: raw pointer casts of trait objects cannot extend lifetimes`). Keep `'a` in the stored
type. Read [references/variance-tables.md](references/variance-tables.md) when you need the exact-owner accessor, or before a
`transmute` that erases the bound: it gives the two facts the `SAFETY` comment must state.

## Coerce the message, not the handle

`std::sync::mpsc::Sender<T>` is invariant in `T`, so the first call site that demands `'static`
pins the whole channel, and every borrowed source fails with E0597. The message stays covariant:
write each consumer as `Sender<Message<'_>>`, never `Sender<Message<'static>>`, and coerce at the
send site. When a consumer moves the sender into `thread::spawn`, make the element type own its
data; no variance trick helps.

Never repair this with a hand-rolled handle whose only mention of `T` is `PhantomData<fn(T)>`. It
coerces to `Sender<Message<'static>>`, escapes with short-lived values still queued, and drops
them after the borrow ends: a use-after-free under Miri. Read [references/variance-tables.md](references/variance-tables.md) when you
need the rejected and repaired pair, or the unsound channel and its Miri output.

## Variance is public API

Wrapping a field in `Cell`, `RefCell`, `Mutex`, `RwLock`, or any `UnsafeCell` flips the struct
from covariant to invariant, and downstream code that shortened the lifetime stops compiling.
Treat it as a breaking change and record it in the changelog. A `*mut &'a T` field breaks the
same coercion; a `*mut T` field does not. A `Box<dyn Fn(&'a T)>` field in place of `fn(&'a T)`
makes the struct invariant over `'a`. A `&'a mut T` field breaks only a type coercion. Code that
shortens a lifetime through a `&mut` unsize coercion inside `Cell`, `RefCell`, or `UnsafeCell`
needs `rust-version = "1.98"`; run `cargo +<msrv> check` when the MSRV is lower.

Lock the variance of each public type with a lifetime parameter, so `cargo check` fails before a
downstream crate does. Keep the covariant probe as a private function whose name starts with `_`:
`fn _config_is_covariant<'a, 'b: 'a>(c: ConfigV1<'b>) -> ConfigV1<'a> { c }`. To lock intended
invariance, put a `compile_fail` doctest on the public type and run `cargo test --doc`. It passes
on any compile error, a wrong import path included, so keep a compiling twin with the same import.

## Traits match their parameters by equality

A trait bound is an equality constraint, not a subtyping constraint. `F: FnMut() -> &'a str` is
satisfied only by a callable whose `Output` **is** `&'a str`. A named function has its own item
type, whose `Fn` impl fixes `Output = &'static str`, so `names.resize_with(10, service_name)` on a
`Vec<&str>` that holds a local borrow fails with E0597. Read the `note:`: it points at the bound
that made the demand, not at the caller. A trait of your own behaves the same:
`S: Sink<&'static u8>` is not `S: Sink<&'a u8>`.

## Three fixes for a producer whose output lives too long

Prefer fix 1.

1. **Give the producer an unbounded output lifetime**, `fn any_name<'a>() -> &'a str`, whenever
   you own it. In safe code the compiler checks the body for every `'a`; over `unsafe` code the
   same signature is the [unbounded-lifetime](#unbounded-lifetimes-come-from-a-raw-pointer) hazard.
2. **Wrap the call**, `|| static_name()`, when the producer is in another crate. Inference gives
   the fresh closure the short `Output`.
3. **Coerce to a function pointer**: `let f: fn() -> &'static str = static_name;`. `fn` pointers
   are covariant in the return type; `fn` items are not. Write the lifetime out:
   `fn() -> &str` gives `error[E0106]: missing lifetime specifier`.

## A free lifetime in `dyn Trait` makes it invariant

A lifetime written **inside** a trait object's parameters matches by equality, so `Box<dyn Fn(&Event<'a>)>` makes a struct invariant in `'a`, where a
`fn(&Event<'a>)` field kept it contravariant. Keep the object
higher-ranked: `Box<dyn for<'x> Fn(&Event<'x>)>`, which elision inside `dyn Fn(&Event)` also
produces, drops the struct's lifetime parameter and serves every caller. Write a free lifetime
into a trait object's parameters only when you intend invariance.

## A `&mut` parameter pins the caller's lifetime

`fn push_str_ref<'a>(v: &mut Vec<&'a str>, s: &'a str)` on a `Vec<&'static str>` local gives
E0597 at the annotation. Drop the `'static` from it, or take the container by value and return
it (`fn with_str_ref<'a>(mut v: Vec<&'a str>, s: &'a str) -> Vec<&'a str>`) so covariance
applies. Prefer the by-value form, or an owned element type, in a public API.

## Related skills

When installed: `rust-unsafe` owns raw-pointer validity, `rust-compiler-errors` covers E0597,
E0521, and E0106 beyond these shapes, `rust-callback-bounds` picks the callable bound, and
`rust-send-sync` owns the auto-trait half of the `PhantomData` table.

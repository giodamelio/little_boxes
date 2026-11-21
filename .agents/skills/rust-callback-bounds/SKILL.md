---
description: Use when choosing or repairing the bound on a callback parameter or a stored callable — Fn(&T) -> K, a for<'a> HRTB, a sort_by_key key projection, or a struct field that holds a closure (generic F, Box<dyn Fn>, fn pointer field). Triggers on "lifetime may not live long enough" from a closure, "one type is more general than the other", "borrowed data escapes outside of closure", "callback returns a reference", "store a closure in a struct", "function item types cannot be named directly", E0747, E0562, or "reached the recursion limit while instantiating".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-callback-bounds
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 66196e9aa1cb478b5a6ded7f3e8236e879a5ef4c
name: rust-callback-bounds
---
# Rust callback bounds

`for<'a> FnMut(&'a T) -> &'a K` **is legal and compiles on stable**. Only a *separate* generic
parameter, as in `FnMut(&T) -> K`, cannot name the higher-ranked lifetime. For a callback that returns a
plain borrow of its argument, do not reach for a GAT, a macro crate, or `Box<dyn Fn>`.

Every quoted diagnostic comes from rustc 1.98.1, edition 2024, on aarch64-apple-darwin.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| Choosing a bound and nothing has failed yet | [Pick the bound from the return type](#pick-the-bound-from-the-return-type) |
| `error: lifetime may not live long enough`, `return type of closure is &'2 ...` | [A free type parameter cannot name 'a](#a-free-type-parameter-cannot-name-a) |
| Callback must return a borrow of its argument | [Return a borrow of the argument](#return-a-borrow-of-the-argument) |
| `error[E0521]: borrowed data escapes outside of closure` | [for<'a> is a no-escape promise](#fora-is-a-no-escape-promise) |
| `error[E0308]: ... one type is more general than the other` | [Closure inference is positional](#closure-inference-is-positional) |
| `error[E0282]: type annotations needed` on a closure argument | [Closure inference is positional](#closure-inference-is-positional) |
| E0282 or `implementation of AsyncFn is not general enough` on `\|o\| async move { .. }` | Write `async \|o\| ..`; see [Pick the bound](#pick-the-bound-from-the-return-type) |
| `error[E0309]` then `error[E0621]` then `error[E0502]` on one function | [Do not hoist the lifetime](#do-not-hoist-the-lifetime) |
| `sort_by_key` rejects `\|o\| &o.field` | Read [key-projections.md](references/key-projections.md#change-what-the-callback-receives) |
| Callback output is generic over `'a` and your signature cannot name it | Read [key-projections.md](references/key-projections.md#generic-outputs-need-rpitit-or-a-gat) |
| Deciding between `F: Fn`, `Box<dyn Fn>`, and `fn(..)` in a field; `error[E0747]: constant provided when a type was expected`, or `error[E0562]: impl Trait is not allowed in field types` | Read [storing-callables.md](references/storing-callables.md#store-a-callable-in-a-field) |
| `expected an Fn() closure, found Arc<dyn Fn()>` | Read [storing-callables.md](references/storing-callables.md#arc-and-rc-of-dyn-fn-do-not-implement-fn) |
| `reached the recursion limit while instantiating` on a writer or visitor trait | Read [storing-callables.md](references/storing-callables.md#by-value-impl-trait-plus-a-delegating-impl-is-a-monomorphization-bomb) |
| A `move` closure unexpectedly implements `Fn`, captures a whole owner, or changes drop timing | Read [closure-capture-semantics.md](references/closure-capture-semantics.md) |
| New E0502, E0505, or E0506 on a closure after a toolchain bump, from `match`, `if let`, or `let` | Read [closure-capture-semantics.md](references/closure-capture-semantics.md#capture-precision-depends-on-the-projection) |
| The callback is `async` | [Pick the bound](#pick-the-bound-from-the-return-type): `AsyncFn`, or a boxed `Send` future |

## Pick the bound from the return type

Read the row that matches what the callback returns. The argument is always `&T`.

| The callback returns | Write this bound | Accepts a bare closure |
| --- | --- | --- |
| Nothing borrowed, no reference at all | `impl Fn(T) -> U` | Yes |
| One concrete type, named in your signature | `impl Fn(&T) -> String` | Yes |
| A free type parameter `K` of the outer function | `impl Fn(&T) -> K` | **No** for a projection that borrows from the argument. A projection through an inner reference (`T = &U`) is accepted |
| A borrow of the argument | `impl for<'a> Fn(&'a T) -> &'a K`, plus `K: ?Sized` | Yes |
| A composite over `'a` your signature can name | `impl for<'a> FnMut(&'a T) -> Reverse<&'a str>` | Yes |
| An output that stays a type parameter over `'a` | Your own trait with RPITIT, or with a GAT | No. Pass a named unit struct |
| An async result, awaited on the caller's task | `impl AsyncFn(&T) -> R` (Rust 1.85+) | Yes, as `async \|o\| ..`. `\|o\| async move { .. }` fails, annotated or not (Rust 1.99.1) |
| A future that must be `Send` because the task that awaits it is spawned (`tokio::spawn`), or a callback stored as `Box<dyn ..>` | `impl for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = R> + Send + 'a>>` | Yes, when the body is `Box::pin(async move { .. })` |

`AsyncFn` has no stable way to bound its future as `Send`, and `Box<dyn AsyncFn(&T) -> R>` is
E0038; a generic field `F: AsyncFn(&T) -> R` compiles. A future that borrows `&T` is not `'static`, so spawning
the callback's own future fails with E0521; take the argument by value or as `Arc<T>` for that.
The `rust-async-internals` skill, when installed, owns the other async callback rules; for
example, a lock held across `.await` needs `tokio::sync::Mutex`.

Add `K: ?Sized` on every `-> &'a K` row. Without it `|o| o.name.as_str()` and
`|b| &b.bytes[..]` fail, because `str` and `[u8]` are not `Sized`.

## for<'a> is a no-escape promise

`for<'a>` on a callback bound is machine-checked: the callback cannot store the reference
anywhere that outlives the call. E0521 means the caller's closure wants to keep a reference that
your API promised it would not keep. Fix the caller, not the bound. Never widen `for<'a>` to a
fixed `'a` (`impl FnMut(&'a T) -> K` with `arr: &'a [T]`): when the body does not mutate the
collection, it compiles and silently permits the store from then on. Read
[hrtb-diagnostics.md](references/hrtb-diagnostics.md#a-fixed-a-permits-the-escape) when you need
the side-by-side example and the full E0521 text.

E0521 also fires for `'static` escapes out of `thread::spawn`. That variant is a different cause
with a different fix; see the `rust-compiler-errors` skill.

## Verify

A bound error appears at the call site, not in your signature. `cargo check` on the library alone
proves nothing about the closures callers write.

| Claim | Check |
| --- | --- |
| The bound accepts the closures callers write | A caller test or doctest that passes inline, unannotated closures: `\|o\| &o.field`, `\|o\| o.name.as_str()`, and one that captures a local. Run `cargo test` |
| The API promises no escape | A `compile_fail` doctest whose closure stores the argument in an outer variable (E0521); a widened `'a` makes it compile and fail. Stable rustdoc ignores the error code, so pair it with a passing doctest that differs only in the store line |
| A size or allocation claim for a stored callable | A `size_of` assertion, or the size and counting-allocator probes in [storing-callables.md](references/storing-callables.md#field-sizes-are-measurable) |
| A writer or visitor trait that takes `impl Trait` | A test that calls the method with a concrete writer (`t.ser(Vec::new())`), then `cargo test` or `cargo build --all-targets`. A library-only `cargo build`, `cargo check`, and `cargo clippy` miss the recursion-limit error |
| The MSRV accepts the syntax | `cargo +<msrv> check` when `rust-version` is below 1.85 (`AsyncFn`, `async \|..\|`) or 1.87 (`use<..>` in a trait) |

## Checklist

- No `'a` was hoisted onto the function, and no `for<'a>` was widened, to silence a closure error.
- `sort_by`, `iter().max_by_key`, or `Vec<&T>` was considered before any projection trait.
- A public generic type that stores behaviour declares its own trait with a blanket `FnOnce` impl.
  No public struct hands users a value through `-> S<impl Fn(..)>`.
- Every [Verify](#verify) row the change touches passed.

## A free type parameter cannot name 'a

`impl FnMut(&T) -> K` desugars to `for<'a> FnMut(&'a T) -> K`. The binder introduces `'a`, but
`K` was fixed at the outer function's scope, before `'a` existed. No closure that returns a borrow
of its argument can satisfy it.

`std` has the same shape. `Vec::sort_by_key` is `FnMut(&T) -> K`, so `v.sort_by_key(|o| &o.country)`
reports the same error. No annotation on the closure repairs it. The bound is wrong, not the
closure. Read [hrtb-diagnostics.md](references/hrtb-diagnostics.md#a-free-type-parameter-cannot-name-a)
when you need the failing code and the full diagnostic.

## Do not hoist the lifetime

Hoist `'a` into `impl FnMut(&'a T) -> K` and rustc reports E0309 (add `T: 'a`), E0621 (add
`arr: &'a mut [T]`), and E0502. Each `help:` is locally correct; follow both and E0502 remains,
because a fixed `'a` keeps every element borrowed past the loop body. Stop at the first error,
restore the elided argument, and pick a row from the decision table. Read
[hrtb-diagnostics.md](references/hrtb-diagnostics.md#the-hoisted-lifetime-dead-end) when you need
the terminal code and the three diagnostics.

## Return a borrow of the argument

Put the output inside the binder: `mut key: impl for<'a> FnMut(&'a T) -> &'a K` with
`K: Ord + ?Sized`. The bound accepts an unannotated closure such as `|o| &o.country`. The
`&mut [T]` receiver stays mutable inside the body, because the higher-ranked `'a` ends at each
call and no borrow of an element survives to block `arr.swap`. Read
[hrtb-diagnostics.md](references/hrtb-diagnostics.md#a-borrow-returning-bound-in-use) when you
need a complete function and caller, or a second reference argument.

Write the binder explicitly when the callback takes two references: elision refuses that with
`error[E0106]`.

## Closure inference is positional

A reference-projecting closure must be written **inline at the call site**. Bind the identical
text to a `let` first (`let g = |o: &Order| &o.country; register(g);`) and it stops satisfying the
identical bound: `lifetime may not live long enough` on the `let`, then
`error[E0308]: ... one type is more general than the other` on the call. The call site supplies a
higher-ranked expectation; a bare `let` supplies none. Annotate the `let`
(`let g: fn(&Order) -> &String = |o| &o.country;` for a closure that captures nothing,
`&dyn for<'a> Fn(&'a Order) -> &'a String` for a capturing one), or route it through one generic
identity function for the whole crate:

```rust
fn hrtb_ref<In, Out: ?Sized, F>(f: F) -> F
where
    F: for<'a> FnMut(&'a In) -> &'a Out,
{
    f
}

struct Order { country: String }
fn register<F: for<'a> FnMut(&'a Order) -> &'a String>(_f: F) {}

fn main() {
    let g = hrtb_ref::<Order, String, _>(|o| &o.country);
    register(g);
}
```

Do not add a macro crate for this, and do not write a per-type `force_hrtb`. The turbofish is
needed only when the surrounding call leaves `In` or `Out` ambiguous.

Only an `Fn`, `FnMut`, or `FnOnce` obligation supplies the expectation. With the bound on your
own trait (`where for<'a> F: MyTrait<&'a T>`), **every** closure form fails, annotated or not.
Pass a named `fn`, whose item type is already higher-ranked, or wrap the closure in
`hrtb_ref(..)`. Read [hrtb-diagnostics.md](references/hrtb-diagnostics.md) when you need the
worked examples and the full diagnostics.

## Related skills

When installed: `rust-discipline` reviews stored callbacks (`Box<dyn Fn>` over `Box<dyn FnMut>`,
`&'a mut` fields), `rust-compiler-errors` owns general E0521 and E0277 triage,
`rust-iterator-impl` owns lending iterators, and `rust-performance` measures the
build cost of a by-value `impl Trait` (`cargo build --timings`).

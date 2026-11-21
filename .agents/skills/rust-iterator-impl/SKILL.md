---
description: Use when writing a custom iterator or making a type iterable, to implement Iterator, IntoIterator, FromIterator, Extend, size_hint, ExactSizeIterator, or DoubleEndedIterator. Triggers on a for loop or collect that rejects a container, a lending iterator and E0207, an ExactSizeIterator len panic, an unconditional_recursion warning in into_iter, E0658 on impl Trait in an associated IntoIter type, enumerate().rev() needing ExactSizeIterator, and iter::from_fn in place of a nightly gen block. Not for calling-side combinator style (rust-code-style) or iteration speed (rust-hot-path).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-iterator-impl
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: acaa2a81e4adab499cff9e4ed1f17a01c73e2f32
name: rust-iterator-impl
---
# Rust iterator impl

Every error text and number below comes from rustc 1.98.1, edition 2024, on
aarch64-apple-darwin.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| `error[E0277]: &Bag is not an iterator` on `for v in &bag`, or E0658 on `type IntoIter = impl ..` | [The three IntoIterator impls](#the-three-intoiterator-impls) |
| `warning: function cannot return without recursing`, then `has overflowed its stack` | [Never call self.into_iter() in the impl](#never-call-selfinto_iter-in-the-impl) |
| `error[E0277]: a value of type X cannot be built from an iterator` | [A collection newtype needs four impls](#a-collection-newtype-needs-four-impls) |
| `error[E0207]: the lifetime parameter 'a is not constrained` | [Iterator cannot borrow from itself](#iterator-cannot-borrow-from-itself) |
| A panic inside `core/src/iter/traits/exact_size.rs` | [size_hint is a contract](#size_hint-is-a-contract) |
| `error[E0277]: ... ExactSizeIterator is not satisfied`, pointing at `.rev()` | [Adapter order](#adapter-order-enumeraterev-needs-exactsizeiterator) |
| `error[E0658]: gen blocks are experimental`, or E0432 on `std::ops::Generator` | [Stateful generators](#stateful-generators-use-iterfrom_fn) |
| The impls are correct, and iteration is slow | the `rust-hot-path` skill; for a known length, `with_capacity` plus `extend` |

## Verify

Set these lint levels once (`[lints.*]` in a single-package manifest):

```toml
[workspace.lints.rust]
unconditional_recursion = "deny"

[workspace.lints.clippy]
iter_without_into_iter = "warn"
into_iter_without_iter = "warn"
```

While you iterate, run `cargo test --locked -p <crate>`. Before commit, run
`cargo clippy --locked --workspace --all-targets -- -D warnings`. The `rust-lints` skill, when installed,
owns the full `[workspace.lints.clippy]` tables and the workspace gate.

| Check | Catches |
| --- | --- |
| `unconditional_recursion = "deny"` | An `into_iter` body that calls itself directly |
| `clippy::iter_without_into_iter`, `clippy::into_iter_without_iter` (pedantic, allow by default) | An exported `iter` or `iter_mut` without its `IntoIterator for &T` or `&mut T`, and the reverse |
| `clippy::should_implement_trait` (style, warn by default) | An inherent `fn next` or `fn into_iter` on an exported type |
| A test that calls `next()` once, then asserts `size_hint()` and `len()` | A wrapper that drops `size_hint`, and an off-by-one after partial use |
| A test on `(index, element)` pairs | A swap of `enumerate().rev()` and `rev().enumerate()` |
| A test that alternates `next` and `next_back` to the end | A `next_back` that yields one element twice or skips one |

A green lint run proves little. The clippy lints skip a type that is not exported, and no lint
sees a missing `IntoIterator for T`, `FromIterator`, or `Extend`, a wrong `size_hint`, or
recursion through a second function. The tests and the [review checklist](#review-checklist)
cover those.

## Review checklist

The change is done when the Verify commands pass and a reading confirms each item. Each has its
home in the section named after it.

- `IntoIterator` for `T`, `&T`, and, with mutable element access, `&mut T` on every container
  a `for` loop iterates; `FromIterator` and `Extend` on a collection newtype.
- No inherent `fn into_iter` or `fn next`; no `into_iter` body that reaches `self.into_iter()`,
  directly or through a helper.
- A `Deref` only to `[T]` or `str`.
- A forwarded or computed `size_hint` on every wrapper, and an exact one under every
  `ExactSizeIterator` impl.
- No `enumerate().rev()` after `filter`, `flat_map`, `take_while`, `chars`, or `chain`.
- No raw pointer or `RefCell::as_ptr` escape from E0207.
- No nightly `gen` block.

## The three IntoIterator impls

The `for` loop and every `I: IntoIterator` bound resolve through the trait, so a container that
offers only an inherent `fn iter(&self)` fails with `error[E0277]: &Bag is not an iterator`.
Rewriting the call site to `b.iter()` hides the defect. Write all three impls once, on the
container: `&'a C`, `&'a mut C`, and `C`. Read [references/impl-examples.md](references/impl-examples.md#the-three-intoiterator-impls-on-one-container)
when you write the impls: it has a run-tested container with all three, a forwarding wrapper
iterator, and the inherent `iter` and `iter_mut` they delegate to.

Rules:

- Do not add an inherent `fn into_iter`; `clippy::should_implement_trait` asks you to delete it.
- Keep the inherent `iter` and `iter_mut`. The borrowing trait impls delegate to them.
- A newtype that yields exactly the inner iterator's items forwards `size_hint` and each trait
  of the inner iterator: `DoubleEndedIterator`, `ExactSizeIterator`, `FusedIterator`, `Clone`.
  Forwarding only `next` reports `(0, None)`: `collect` loses the capacity hint and `len()`
  panics.
- A wrapper that drops, adds, or merges items computes its own `size_hint`, and claims
  `ExactSizeIterator` only when that hint is exact.
- Give `IntoIter` a named public type when the caller may store it. A std type such as
  `std::slice::Iter<'a, u32>` in a `pub` signature ties the API to it forever.
- `type IntoIter = impl Iterator<Item = T>;` fails on stable with E0658 (issue 63063). Name the
  std type, write a wrapper, or use `Box<dyn Iterator<Item = T> + 'a>` (one allocation and a
  dynamic call per item). A non-capturing closure coerces to a `fn` pointer, so
  `std::iter::Map<std::slice::Iter<'a, T>, fn(&'a T) -> U>` is nameable.

## Never call self.into_iter() in the impl

A body `fn into_iter(self) -> Self::IntoIter { self.into_iter() }` compiles and links, warns
only with `unconditional_recursion`, and aborts at run time with a stack overflow. It usually
works when written, because an inherent `fn into_iter(self)` shadows the trait method; delete
that inherent method, as `clippy::should_implement_trait` asks, and the same body calls itself.
Read [references/diagnostics.md](references/diagnostics.md#the-selfinto_iter-trap) when you need
the code and the exact warning and abort text.

The `unconditional_recursion = "deny"` level in [Verify](#verify) turns the warning into a build
error. Without that level, search for the shape:
`rg -nU 'fn into_iter\(self\) -> [^{]*\{\s*self\.into_iter' --type rust`. Keep `-U`: rustfmt splits the
body across lines.

## A collection newtype needs four impls

`Deref` does not supply `IntoIterator`, `FromIterator`, or `Extend`: method-call syntax follows a
deref; trait resolution does not. A `Deref<Target = Vec<T>>` newtype still fails `for v in w`,
`collect`, an `I: IntoIterator` argument, `w.into_iter()` (E0507), and `w.extend(..)` (E0596;
`DerefMut` fixes only that one). Read
[references/diagnostics.md](references/diagnostics.md#a-deref-newtype-is-not-a-collection) when
you need the exact error for each call site.

Read [references/impl-examples.md](references/impl-examples.md#a-collection-newtype-with-four-impls)
when you write them: it has a run-tested newtype with the four impls and a `Deref<Target = [T]>`.

`Deref<Target = [T]>` costs the caller nothing and keeps the growth API private (`b.push(4)` is
E0599). Do not add `DerefMut` to get mutation back; add one named inherent method for each
mutation the type really offers.

## Iterator cannot borrow from itself

`Item` cannot name the lifetime of `&mut self`, so an iterator that owns its buffer cannot yield
references into it. rustc gives E0207, then `lifetime may not live long enough`; the `help` line
misleads, and no attribute, bound, or `PhantomData` field fixes it. Pick one of three options:

1. Borrow the data: `struct Frames<'a> { data: &'a [u8], pos: usize }` with
   `type Item = &'a [u8]`.
2. Yield a guard that owns its borrow, such as `Ref<'a, T>` from a borrowed `&'a [RefCell<T>]`.
3. Drop the `Iterator` impl for an inherent `fn next_frame(&mut self) -> Option<&[u8]>`, used with
   `while let`. Do not name it `next`: `clippy::should_implement_trait` flags it.

Read `references/lending-iterators.md` when you implement one: it has a tested example of each
option, the full E0207 output, and a lending trait with a generic associated type.

Never escape the lifetime through a raw pointer or `RefCell::as_ptr`. That is unsound: the
yielded reference outlives the tracked borrow, so a caller can hold it across a `push` that
reallocates. Change the API. A clean `cargo +nightly miri test --locked` run proves nothing
here; Miri reports the defect only when a test interleaves the reference with a write.

## size_hint is a contract

The default `size_hint` is `(0, None)`. Two consequences.

**`collect` falls back to the growth ladder** (500 unhinted items land in capacity 512 on
1.99.1). For a known output length, use `with_capacity` plus `extend`.

**`ExactSizeIterator` panics at run time.** `impl ExactSizeIterator for X {}` compiles with no
`size_hint` override, and the default `len()` then panics inside
`core/src/iter/traits/exact_size.rs`.

Override `size_hint` before you implement `ExactSizeIterator`. Implement `DoubleEndedIterator` as
well when the sequence has a defined end, because `.rev()` and `.enumerate().rev()` both need
it. Read [references/impl-examples.md](references/impl-examples.md#an-exact-size-double-ended-iterator)
when you write the impls: it has a run-tested iterator with both.

Rules for the two traits:

- Implement `ExactSizeIterator` only when the count is known before iteration and fits `usize`;
  a filtered or input-driven source does not qualify.
- `next_back` yields the same elements as `next`, from the other end, and the two must not hand
  out one element twice where they meet.
- The `size_hint` lower bound is a promise, not a guess: an over-large one reserves memory that
  never fills.

## Adapter order: enumerate().rev() needs ExactSizeIterator

`Enumerate<I>` implements `DoubleEndedIterator` only when `I: DoubleEndedIterator +
ExactSizeIterator`. An adapter that loses the exact length breaks a later `.rev()`, and the
E0277 blames `.rev()`, not the adapter. A follow-on E0599 goes away with it.

Swapping to `rev().enumerate()` compiles and renumbers from 0 at the tail, where
`enumerate().rev()` keeps the forward index. The elements are the same, so assert on the
`(index, element)` pairs.

Read `references/adapters-and-generators.md` when you need to know which adapter keeps the exact
length, or want the full error and the three orderings side by side.

## Stateful generators: use iter::from_fn

Coroutines and `gen` blocks are still nightly on 1.99.1 (E0658, and E0554 for
`#![feature(coroutines)]`). Pinning `rust-toolchain.toml` to nightly for one iterator pins the
whole workspace. `use std::ops::Generator;` gives E0432 on every channel.

`std::iter::from_fn` is the stable shape. A `move` closure owns the state, and the function
returns `impl Iterator`. No allocation, no `Pin`, no `Box<dyn>`.

`FromFn` is never `DoubleEndedIterator` or `ExactSizeIterator`. Read
`references/adapters-and-generators.md` when you write a `from_fn` generator, port an old
`Generator` snippet, or need `successors`, `repeat_with`, or the `FromFn` cost table.

## Related skills

When installed, `rust-compiler-errors` covers E0277, E0507, and E0596 beyond the shapes here.

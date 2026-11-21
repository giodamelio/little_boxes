---
description: Use when adding Pin to a signature, writing a self-referential struct or other address-sensitive type, or projecting a pinned reference into a field (pin projection, structural pinning) by hand or via pin-project-lite. Also for choosing between std::pin::pin!, Box::pin, and Pin::new_unchecked; PhantomPinned and Unpin semantics; PinnedDrop; and errors such as "PhantomPinned cannot be unpinned", E0596 on a Pin, or an E0308 at a pin! call that says expected S, found &mut S (1.97 and 1.98 name unreachable_pin_macro_type_constraint). Not for polling, wakers, or cancel safety; use rust-async-internals.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-pin-projection
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 5359739098e04760f5a8a7f3cb7a203631fad220
name: rust-pin-projection
---
# Rust pin projection

`Pin<&mut T>` enforces nothing when `T: Unpin`, and `!Unpin` never makes a type unmovable. The
restriction starts at the pin, and it applies only to a `!Unpin` type.

Diagnostics are from rustc 1.99.1 (edition 2024), pin-project 1.1.13, pin-project-lite 0.2.17,
and Miri on nightly 2026-05-15.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| You are about to write `self: Pin<&mut Self>` | [Pin enforces nothing](#pin-enforces-nothing-for-an-unpin-target) |
| `PhantomPinned`, or a claim that a type "cannot move" | [Unpin does not mean movable](#unpin-does-not-mean-movable) |
| `error[E0277]: PhantomPinned cannot be unpinned` | [Get a value pinned](#get-a-value-pinned) |
| `error[E0515]` or `error[E0716]` at a `pin!` | [Compile errors at a pin](#compile-errors-at-a-pin) |
| `error[E0308]` at `pin!(s)`: ``expected `S`, found `&mut S` `` (1.97 and 1.98 also name `unreachable_pin_macro_type_constraint`), or `E0382` on a `Pin<&mut T>` | [Compile errors at a pin](#compile-errors-at-a-pin) |
| Miri: `allocN has been freed, so this pointer is dangling` | [Never pin a named stack binding](#never-pin-a-named-stack-binding) |
| `error[E0133]` from a `macro_rules!` that calls `Pin::new_unchecked` | [Never pin a named stack binding](#never-pin-a-named-stack-binding) |
| Miri: `tag does not exist in the borrow stack`, invalidated by a `mem::swap`; or you wrote `impl<T> Unpin for MyType<T> {}` | Obligation 1 in [Structural pinning](#structural-pinning-the-four-obligations) |
| `impl Drop` or `#[repr(packed)]` on a type with a pinned field | Obligation 2 |
| A pinned value sits in `ManuallyDrop`, or its storage is overwritten or freed before its destructor runs | Obligation 3 |
| `error[E0596]: cannot borrow data in dereference of Pin<&mut T> as mutable` | Obligation 4 |
| You choose between `pin-project` and `pin-project-lite` | [The projection macros](#the-projection-macros) |
| You write a smart pointer that a caller may wrap in `Pin` | [Your own pinning pointer type](#your-own-pinning-pointer-type) |
| You must overwrite a pinned value | Use `Pin::set`: it is safe and drops the old value in place. Read `references/pin-projection-macros.md` (section "Assign into a pinned value") for the example |
| You must prove a pinned type sound, or stay `!Unpin` | [Verify](#verify) |
| You review code that pins or projects | Read `references/pin-projection-macros.md` (section "Review checklist") |

## Verify

| Claim | Check | What a pass does not prove |
| --- | --- | --- |
| No pinned value moves, is overwritten, or loses its destructor | When a nightly toolchain with the `miri` component is installed, run `cargo +nightly miri test --locked` on the crate that holds `Pin::new_unchecked`, `map_unchecked_mut`, `get_unchecked_mut`, a manual `Unpin`, or a manual `Drop` on a pinned type. Run it before you merge such a change, and in CI for that crate. Without Miri, report the pinning code as not checked by Miri | Paths the tests do not run. A pinning defect stays silent until code uses a stored pointer to the pinned value, so a test must pin the value, store the pointer, do the operation under test (move, swap, drop, overwrite), and then use the pointer |
| An address-sensitive type stays `!Unpin` | A `compile_fail` doctest, or a `trybuild` case, that calls `assert_unpin::<T>()`. Read `references/pin-projection-macros.md` (section "Prove the Unpin result with a test") when you write the test | A `compile_fail` doctest passes on any compile error, a typo included. `trybuild` compares the full error text |
| A type you promise is `Unpin` stays `Unpin` | A normal test that calls `assert_unpin::<T>()` | Anything about soundness |
| It builds | `cargo build`, `cargo test`, `cargo clippy` | Pin soundness |

A change that pins or projects is complete when the Miri row passes (or the report says that Miri
did not run), the `Unpin` tests hold, and the review checklist holds. The `rust-sanitizers-miri`
skill, when it is installed, owns the Miri flags and the aliasing-model policy; the Tree Borrows
lane is `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test --locked`.

## Pin enforces nothing for an Unpin target

Check this before you add `Pin` to any signature. `Pin::new` and `Pin::get_mut` are both safe
for an `Unpin` target, and `Pin<Ptr>` implements `DerefMut` for one, so safe code moves the value
out through any of them.

- Write `Pin` in a signature only when the type is `!Unpin`, or when a trait such as `Future`
  forces the receiver. Otherwise take `&mut self`.
- Add a `PhantomPinned` field to the type whose address must stay stable. That one field
  removes the auto `Unpin` impl and turns the API from documentation into enforcement.

## Unpin does not mean movable

Every Rust type moves, `!Unpin` included: assignment, `mem::swap` through `&mut`, `Box::new`, and
`vec![..]` all move a `PhantomPinned` type with no `unsafe` and no warning. Only trait bounds read
`Unpin`: `Pin::new`, `Pin::get_mut`, `DerefMut for Pin<Ptr>`, and impls such as
`Future for &mut F`. Read `T: !Unpin` as "once a value is pinned, safe code cannot reach `&mut T`
again". Read `references/pin-projection-macros.md` (section "Moves that Pin does not stop") when a
design depends on a type that "cannot move".

## Get a value pinned

| Constructor | Result | Unsafe | Escapes the frame | Allocates |
| --- | --- | --- | --- | --- |
| `std::pin::pin!(value)` | `Pin<&mut T>` | no | no | no |
| `Box::pin(value)` | `Pin<Box<T>>` | no | yes | yes |
| `Pin::new(&mut value)` | `Pin<&mut T>` | no, needs `T: Unpin` | no | no |
| `unsafe { Pin::new_unchecked(&mut value) }` | `Pin<&mut T>` | yes | no | no |

`std::pin::pin!` is stable since 1.68 and needs no crate. `Pin::new` on a `!Unpin` type fails
with ``error[E0277]: `PhantomPinned` cannot be unpinned`` and the advice to use `pin!` or
`Box::pin`. Take that advice. Do not reach for `Pin::new_unchecked`. The examples use `SelfRef`, a
struct whose `ptr` field points into its own `buf` field; read `references/pin-projection-macros.md`
(section "The SelfRef example") for its definition.

## Never pin a named stack binding

`Pin::new_unchecked(&mut local)` commits the value for the rest of its life, but the `Pin` drops
first, and safe code then moves the local. Cargo, rustc, and Clippy stay silent; Miri reports
`allocN has been freed, so this pointer is dangling`.

- Never call `Pin::new_unchecked` on a binding you can still name. Use `std::pin::pin!`, which
  puts the value in an unnameable temporary, or `Box::pin`, which owns it. `Drop::drop` is the
  one exception: there the value is never used again after the call.
- Call `Pin::new_unchecked` only on a pointer you own for the whole life of the value.
- Never hand-roll a stack pinning `macro_rules!`. Wrapping its call in `unsafe { .. }` to fix
  `E0133` scopes the shadow to that block, and the original binding moves out. Delete the macro
  and use `std::pin::pin!`.

Read `references/structural-pinning.md` (section "A named stack binding") when you must explain
either failure in a review.

## Structural pinning: the four obligations

A projection `Pin<&mut Self> -> Pin<&mut Field>`, written with `Pin::map_unchecked_mut`, makes the
field structurally pinned. A field that is not structurally pinned projects to a plain `&mut`
through `Pin::get_unchecked_mut`. Choose per field, and state the choice in the `SAFETY:` comment.
Break one of these obligations, and safe callers get undefined behaviour:

1. **Structural Unpin.** Never write `impl Unpin` by hand on a type that has a pinning
   projection. It needs no `unsafe`, so it passes an unsafe-focused review, and it makes
   `Pin::get_mut` safe. Delete the impl, and let the auto trait decide.
2. **Pinned destruction.** `Drop::drop` gets `&mut self` for a pinned value. Write the destructor
   as if the receiver were `Pin<&mut Self>`: use `PinnedDrop` under a macro, or an inner function
   that takes the pin. Reject `#[repr(packed)]` on a pinned type yourself: drop glue can move a
   packed field, and rustc does not warn.
3. **Notice of destruction.** Never free, overwrite, or reuse the storage of a pinned `!Unpin`
   value before its destructor runs: no `ManuallyDrop` around it, no `ptr::write` over it, no
   `Vec::set_len` below it. Call `ptr::drop_in_place` first. `Pin::set` is sound. A destructor
   that can panic part way must abort instead; by default, make each `Drop` infallible.
4. **No move-out operation.** Expose no `take`, `replace`, or `swap` on a structurally pinned
   field. The type system rejects them (`E0596`) once the projection returns `Pin<&mut Field>`.

Read `references/structural-pinning.md` when you write or review a projection, a destructor, or
storage of pinned values.

## The projection macros

Write the projection with a macro. A hand-written `map_unchecked_mut` is correct only while all
four obligations hold, and no build step checks that. Take `pin-project-lite` when the dependency
graph builds no proc-macro crate yet. Take `pin-project` for a tuple struct or tuple variant,
for `UnsafeUnpin`, or for its rejection of every `#[repr(packed)]` struct. Read
`references/pin-projection-macros.md` (section "Choose the crate") when you choose.

Both macros replace the auto `Unpin` impl with one that reads the `#[pin]` fields alone. A
`PhantomPinned` field without `#[pin]` therefore leaves the struct `Unpin`, with no diagnostic.
Add `#[pin]` to the `PhantomPinned` field, or put `#[project(!Unpin)]` (`pin-project-lite`) or
`#[pin_project(!Unpin)]` (`pin-project`) on the type. Hold that result with a compile-fail test.

## Your own pinning pointer type

`Pin<P>` pins `P::Target` only when `P` is a real indirection: a `struct Inline<T>(T)` whose
`Deref` returns `&self.0` moves the "pinned" data with every move of the `Pin<P>`. Read
`references/pin-projection-macros.md` (section "A pinning pointer that pins nothing") for the
three rules such a type must keep.

## Compile errors at a pin

| Error | Fix |
| --- | --- |
| `E0716` or `E0515` at a `pin!` passed as an argument or returned | The temporary dies at the end of the statement. Bind first (`let mut p = pin!(v);`), then pass `p.as_mut()`. Use `Box::pin` when the value must outlive the function |
| `E0308` ``expected `S`, found `&mut S` `` at `pin!(s)` with `s: &mut S` | Since 1.97, `pin!(r)` gives `Pin<&mut &mut T>`. Take `Pin<&mut S>`, use `Pin::new(s)` for an `Unpin` target, or take `s: S` by value. Never `unsafe { Pin::new_unchecked(s) }` and never `pin!(&mut s)`: the caller can still move the value |
| ``E0382: use of moved value: `p` `` on a `Pin<&mut T>` | `Pin<&mut T>` is not `Copy` and is not reborrowed implicitly. Call `.as_mut()` before every call except the last, and declare `let mut p` |

Read `references/pin-projection-macros.md` (sections "How long a pin! value lives" and "pin! of a
&mut reference") when the error names a `pin!` nested in a tuple, a struct literal, or `Some(..)`.

## Related skills

When installed: `rust-async-internals` owns polling, `select!`, and cancel safety; `rust-unsafe`
owns the `SAFETY:` comment convention and the `#[repr(packed)]` rules behind E0793; `rust-macros`
owns `macro_rules!` hygiene.

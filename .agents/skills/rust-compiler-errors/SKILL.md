---
description: Use when rustc or cargo fails with a borrow checker, lifetime, trait bound, type inference, or unresolved import error, or any E0xxx code. Codes include E0382, E0499, E0502, E0506, E0507, E0373, E0597, E0716, E0521, E0106, E0277, E0308, E0599, E0282, E0283, E0284, E0425, E0432, E0433, E0038, E0275. Also "does not live long enough", "missing lifetime specifier", "not dyn compatible", or a pasted rustc error. Not for E0207 in an iterator or handler trait (rust-iterator-impl, rust-event-loop-state), E0793 (rust-unsafe), or whether a type is Send (rust-send-sync).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-compiler-errors
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 98b7c8fc121d9e9ea37aea0859185ed98cde7d96
name: rust-compiler-errors
---
# Rust compiler errors

## First moves

```bash
rustc --explain E0499                 # full explanation, offline
cargo build --message-format=short    # one line per diagnostic
# count the error codes in a large failure
cargo build --message-format=json 2>/dev/null | grep -o '"code":{"code":"E[0-9]*"' | sort | uniq -c | sort -rn
```

Fix the first error, then rebuild: most of the cascade after it disappears.

## Prove the fix

- While you iterate, run `cargo check` with the package, features, and target that failed. Add
  `--all-targets` when the error came from `cargo test`, a bench, or an example.
- Before you call the error fixed, run the command that failed. `cargo check` and `cargo clippy`
  stop before monomorphization and linking, so they exit 0 on "reached the recursion limit while
  instantiating" and on linker errors.
- A clean build proves the types, not the behavior. Run the test that covers the change.

## Fixes that compile and hide the bug

Each of these removes the error and keeps the defect.

| Reflex | What it hides | Instead |
| --- | --- | --- |
| `.clone()` for E0382 or E0505 | Two owners of an identity, a handle, or a large buffer | [The clone reflex](#the-clone-reflex) |
| `'static` on a field or bound for E0106, E0597, or E0521 | A demand for data that lives forever | Store owned data, or scope the borrow |
| `RefCell` or `Rc<RefCell<T>>` for E0499 or E0502 | A `RefCell already borrowed` panic at run time | Only when a split failed and the graph needs it |
| `Box::leak` for E0521 or E0597 | Memory that grows once per call | `Arc` or `std::thread::scope` |
| `.unwrap()` or `.expect()` for E0308 or E0277, as rustc's `help:` suggests | A panic path | `?`; `.expect("<invariant>")` only for a documented invariant |
| `unsafe`, `unsafe impl Send`, or `transmute` | The bug, and possible undefined behavior | Never for a type or borrow error |
| `#![feature(...)]` for E0658 | E0554 on stable | Never in a stable crate; use a stable alternative or raise the MSRV |
| `cargo add <name>` from `help:` for E0432 or E0433 | rustc suggests it for any unknown path segment; a guessed name can be a malicious package | [Unresolved imports](#e0432-and-e0433-unresolved-imports) |
| A higher `#![recursion_limit]` for E0275 or "while instantiating" | An infinite type chain | The matching triage row |

After three failed attempts at one error, stop and suspect the design: which component owns the
data for its whole lifetime, does the borrow cross a thread, `.await`, callback, or FFI call, and
what would owning it cost?

## Triage table

| Code | Message | What it means | First move |
| --- | --- | --- | --- |
| E0382 | borrow of moved value | Consumed, then used again | Decide the owner; borrow instead of moving |
| E0505 | cannot move out of `x` because it is borrowed | A live borrow outlives the move | Shorten the borrow, or move first |
| E0507 | cannot move out of `x` which is behind a shared reference | Ownership needed, only `&` held | `mem::take`, `mem::replace`, or `Option::take` through `&mut`; a `self` receiver; a clone. See [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md) |
| E0509 | cannot move out of type `T`, which implements the `Drop` trait | `Drop` blocks every partial move | Move `Drop` to a one-field guard type, not the aggregate. See [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md) |
| E0499 | cannot borrow `x` as mutable more than once at a time | Two live `&mut` to one place | `split_at_mut`, index disjointly, or scope one borrow |
| E0502 | cannot borrow `x` as mutable because it is also borrowed as immutable | A read borrow is live across a write | Copy the value out, then mutate |
| E0506 | cannot assign to `x` because it is borrowed | A write while a borrow is live | Finish the read, or borrow fields, not `self` |
| E0502, E0499 | note: this call may capture more lifetimes than intended | Edition 2024: a returned `impl Trait` captures every lifetime in scope | Add `+ use<..>` to the callee's return type when the value does not borrow that argument (1.82+; 1.87+ in a trait); else end the borrow at the caller. Do not clone |
| E0596 | cannot borrow as mutable, as it is behind a `&` reference | The parameter is `&T`, not `&mut T` | Change the signature, not to interior mutability |
| E0373 | closure (or async block) may outlive the current function, but it borrows `x` | A `'static` closure or future (`thread::spawn`) borrows a local | Add `move`, after an `Arc` clone if the caller still needs the data; or `thread::scope` |
| E0597 | `x` does not live long enough | A local is dropped while borrowed | Move the binding outward |
| E0716 | temporary value dropped while borrowed | A temporary ended before its borrow | Name the owner in a `let`; `bar(&foo())` is never extended |
| E0515 | cannot return reference to local variable | The callee owns the data | Return owned, or take a buffer parameter |
| E0521 | borrowed data escapes outside of function | A borrowed parameter reaches a `'static` bound | Pass an `Arc` or owned data, or use `thread::scope` |
| E0310 | the parameter type `T` may not live long enough | A generic `T` reaches a `'static` bound | `T: 'static` for owned data; `dyn Trait + 'a` when it may borrow |
| E0106 | missing lifetime specifier | A struct or return type holds a reference with no stated source | Field: store owned data unless the type is a short-lived view. Return: name the input it borrows from. See [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md) |
| E0277 | the trait bound `T: X` is not satisfied | A trait is missing or not in scope | Import it or add the bound |
| E0277 | the trait bound `!: X` is not satisfied | Never-type fallback (all editions since 1.100) | Name the type, see [the fallback section](#e0282-e0283-e0284-and-the-never-type-need-a-type-anchor) |
| E0277 | `T` cannot be sent between threads safely | A non-`Send` value crosses a thread boundary | If `help:` names `Sync`, a `&T` crossed: use a `Mutex` or an atomic, never `unsafe impl Send` |
| (none) | future cannot be sent between threads safely | A non-`Send` value (`Rc`, a lock guard) lives across an `.await` | The `note:` names both. Drop the value before the `.await`, or use `Arc`; `clippy::await_holding_lock` flags a guard |
| E0308 | mismatched types | Often one reference layer or one `Option` apart | Compare the two types the note prints |
| E0599 | no method named `m` found | Typo, trait not imported, or another crate version | `use` the trait from `help:`; else read the locked source ([Unresolved imports](#e0432-and-e0433-unresolved-imports)) |
| E0599 | the method `m` exists for `T`, but its trait bounds were not satisfied | A type argument lacks a bound the impl needs | Derive the trait the `note:` lists on the inner type, or add the bound |
| E0275 | overflow evaluating the requirement `T: X` | The trait solver hit the recursion limit | A repeating type in the note is an infinite chain: fix the impl. Raise the limit only for a finite, deep type |
| (none) | reached the recursion limit while instantiating | An infinite chain at monomorphization, often `&mut &mut ... W` from recursion on a by-value `impl Write` | `cargo check` exits 0 on it. Take `&mut dyn Write` |
| E0038 | the trait `T` is not dyn compatible | One trait item gets no vtable slot | Read the `...because` note, then [references/dyn-compatibility.md](references/dyn-compatibility.md), which maps each note to its fix |
| E0432, E0433, E0425 (E0412) | unresolved import; cannot find type, module, crate, or value `x` in this scope; before 1.95, failed to resolve | An off feature, a wrong path, a missing `use` or `crate::`, or a missing crate | See [Unresolved imports](#e0432-and-e0433-unresolved-imports) |
| Other codes | E0015, E0072, E0184, E0271, E0367, E0369, E0562, E0603, E0631, E0658, E0740, E0793 | | Read [references/more-codes.md](references/more-codes.md) when the code is not in this table |

## The clone reflex

E0382 says two places want one value. `.clone()` always compiles; it is right for a small owned
copy and wrong for an identity, a handle, a large buffer, or shared state.

Read [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md#who-owns-the-value)
when you decide the owner: it maps each kind of value to its answer.

A clone inside a loop allocates once per iteration. Measure before you remove it:
`cargo bench --locked -p <crate> --bench <b> -- --save-baseline before` (the `rust-performance`
skill, when installed).

## Borrow conflicts are usually a split problem

E0499 and E0502 rarely need interior mutability. Show that two borrows touch different data: `split_at_mut` or `get_disjoint_mut` for indices, field borrows instead of a
`&self` helper, or a copy of the value before the write.

Read [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md) when the triage
row's first move does not fit a borrow error, or to compare interior mutability costs.

## E0282, E0283, E0284, and the never type need a type anchor

The constraints do not select one type; Rust does not infer every input backward from the
result. Add the smallest local anchor:

```rust
let parsed: u64 = "42".parse()?;
let bytes = Vec::<u8>::new();
let converted = u64::from(7_u8);
```

Do not change a public return type, add `'static`, or add a broad trait bound only to silence
inference. Rebuild after the one anchor; later diagnostics are often a cascade.

An unconstrained generic call as a statement, such as `f()?;`, can rely on never-type fallback.
Rust 1.100 falls back to `!` on every edition (``the trait bound `!: Default` is not satisfied``).
Name the type: `f::<()>()?;` or `let () = f()?;`. Allowing the old compatibility lint cannot
restore unit fallback. Read
[references/more-codes.md](references/more-codes.md#the-never-type-fallback) when you need the
programs.

## E0432 and E0433: unresolved imports

A `cargo add` line in `help:` is not a fix. Check in this order and stop at the first match: an
item behind an off feature, a `use` path that `help:` offers, a module of this crate (`crate::`
or `super::`), an item path that differs in the locked version of a listed crate, and only then
a new crate. Read [references/unresolved-imports.md](references/unresolved-imports.md) for the
commands of each step, and always before you add a crate.

## A `Drop` impl changes the borrow checker

`impl Drop` can break code that compiled with E0597, E0502, E0509, or E0184, and no error title
names `Drop`: the drop point is one more use of every borrow the value holds (note: "... runs the
`Drop` code for type `Guard`"). Drop the guard before the read, scope it in a block, or keep the
type `Drop`-free. Read the `Drop`
section of [references/borrow-checker-fixes.md](references/borrow-checker-fixes.md) when an error
follows a new `impl Drop`, or for E0507 inside `drop(&mut self)`.

## Related skills

When installed: `rust-borrow-semantics` owns temporary scopes, `rust-callback-bounds` the
E0309, E0621, E0502 cascade on a `Fn(&T) -> K` parameter, `rust-send-sync` every
`cannot be sent` message, and `rust-iterator-impl` E0207 on an iterator.

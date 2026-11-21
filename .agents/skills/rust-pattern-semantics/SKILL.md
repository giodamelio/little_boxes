---
description: Use when a Rust pattern, match guard, or if let guard decides what moves, borrows, or drops, or when an edition 2024 pattern or let chain fails to compile. Triggers on partial move, binding mode, match ergonomics, ref pattern, let chains, implicitly-borrowing pattern, guard runs twice, E0510, E0594, or E0596 in a guard.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-pattern-semantics
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 371daf19c167ddb1f2fca7fe7f7685cbdcc4dcb6
name: rust-pattern-semantics
---
# Rust Pattern Semantics

## Analysis order

For the pattern that the question or the error names, determine:

1. The crate edition and the MSRV (`rust-version` in `Cargo.toml`). They decide which syntax
   compiles. See [Version gates](#version-gates).
2. Whether the scrutinee is a place or a value expression.
3. The alternatives of each or-pattern.
4. The default binding mode at each nesting level.
5. Whether each binding is by value, `ref`, or `ref mut`, and which fields a partial move leaves
   initialized.
6. When the guard runs, and whether it can run more than once.
7. Where the scrutinee temporaries, the guard temporaries, and the bindings drop.

Do not propose a clone, a wildcard arm, or a `ref` keyword before this pass. Each one can
compile and change what the code moves, borrows, or matches.

## Version gates

| Feature | Needs |
| --- | --- |
| Let chains in `if` and `while` (`if let A = a && cond`) | Edition 2024 and rustc 1.88+ |
| `if let` guards, and a `let` chain inside a guard (`if let P = e && cond`) | rustc 1.95+, every edition |
| `std::assert_matches!` | rustc 1.96+ |
| Match ergonomics reservations | Edition 2024 |

Read `references/edition-2024-patterns.md` when an error from one of these gates appears: it has
the exact message for each toolchain.

Do not add a `#![feature]` gate for an E0658 from this table. Raise `rust-version`, or use the
fallback: a nested `if let`, or `assert!(matches!(..))`. The `rust-compiler-errors` skill lists
other repair hazards.

A `let` chain inside a match guard compiles on edition 2021 (measured on rustc 1.99.1). A chain in
an `if` or `while` condition does not.

## Verify

| Claim | Check | What a pass does not prove |
| --- | --- | --- |
| Binding type or mode | `let () = binding;` and read the E0308 label; or a typed call as in [Binding modes](#binding-modes) | Drop timing or guard repetition |
| Guard count, drop point, lock hold | A `#[test]` or a doctest (it runs by default) that asserts on a counter or on `Mutex::try_lock`, built with the crate's edition | Behavior on another edition |
| Pattern shape in a test | `std::assert_matches!(value, Some(n) if n > 2);` on MSRV 1.96+; else `assert!(matches!(..))` | Fields that the pattern does not name |
| A pattern must not compile | A `compile_fail` doctest that holds only the probe code: `compile_fail,E0xxx` when rustc prints a code (E0004, E0509, E0510, E0594, E0596), a bare `compile_fail` for the uncoded edition 2024 errors. To prove the code on stable, run `cargo check --message-format=short` on the probe and find `error[E0xxx]`, or use a `trybuild` case, which compares the full stderr | The reason for the failure. Stable rustdoc accepts any compile error and ignores a named code; only nightly rustdoc checks it |
| Syntax builds on the MSRV | `cargo +<msrv> check --locked` for a let chain, an `if let` guard, or `assert_matches!` | Behavior, and code behind other features or targets |

`std::assert_matches!` is not in the prelude. Call it by path, or import it with
`use std::assert_matches;`. `use std::assert_matches::assert_matches;` fails with E0432.

## Triage

| Symptom | Likely cause | First action |
| --- | --- | --- |
| Value is partly usable after destructuring | The pattern made a partial move | List moved and initialized fields |
| Partial move fails only after adding `Drop` (E0509) | The destructor needs the whole value | Move `Drop` to a one-field guard, or `mem::take` the field. See [Partial moves](#partial-moves) |
| Guard side effect occurs twice | Overlapping or-pattern alternatives both matched | Remove the side effect or split the arms |
| E0510, or E0594/E0596 with "immutable for the pattern guard" | The guard mutates or mutably borrows the scrutinee or a pattern binding | Move the mutation into the arm body. A signature change does not help |
| Lock or borrow stays held inside the arm | The lock is in the `match` scrutinee or an `if let` guard | Copy the value out first. See [Guard temporaries](#guard-temporaries) |
| Binding became `&T` after a refactor | Match ergonomics changed the default binding mode | Probe the binding type in the target edition |
| `... within an implicitly-borrowing pattern` | Edition 2024 reservation | Apply the fix from the edition 2024 table |
| `let chains are only allowed in Rust 2024 or later` | Let chain in an edition 2021 `if` or `while` | Nest the `if let` and repeat the `else` branch at each level, or migrate to edition 2024 with MSRV 1.88+ (the `cargo-workflows` skill) |
| E0658 on a let chain, an `if let` guard, or `assert_matches!` | The toolchain is older than the [version gate](#version-gates) | Raise `rust-version` or use the fallback. Do not add `#![feature]` |
| Wildcard fixes exhaustiveness but tests fail | The missing variant needed a semantic policy | Add an explicit arm and behavior test |

## Binding modes

A non-reference pattern binds by value. When a non-reference pattern matches a reference, match
ergonomics switches the default binding mode to `ref` or `ref mut`, and the inner bindings
borrow:

```rust
fn shared(_: &String) {}
fn mutable(_: &mut String) {}

let mut value = Some(String::from("x"));
if let Some(text) = &value { shared(text); }
if let Some(text) = &mut value { mutable(text); }
```

A function argument coerces `&mut T` to `&T`, so `shared(text)` proves only that `text` is some
reference. To see the exact type, write `let () = text;`. The E0308 label says
``this expression has type `&String` ``. Remove the line after you read it.

Edition 2024 restricts explicit `mut`, `ref`, `ref mut`, and `&` inside a pattern that borrows
implicitly. See [Edition 2024 match ergonomics](#edition-2024-match-ergonomics).

## Partial moves

A pattern can move one field and borrow another. The original value then cannot be used as a
whole, but fields that were not moved remain usable.

```rust,run
struct Record {
    name: String,
    count: u32,
}

fn main() {
    let record = Record { name: String::from("job"), count: 3 };
    let Record { name, ref count } = record;
    assert_eq!(name, "job");
    assert_eq!(*count, 3);
    assert_eq!(record.count, 3);
}
```

A type with a `Drop` implementation cannot be partially moved because `drop` must receive the
whole value:

```rust,compile_fail,E0509
struct Resource { name: String }
impl Drop for Resource { fn drop(&mut self) {} }

fn main() {
    let resource = Resource { name: String::from("owned") };
    let Resource { name } = resource;
    println!("{name}");
}
```

A consuming method fails the same way: `fn into_name(self) -> String { self.name }` is E0509 on
this type. Take the field out instead: `fn into_name(mut self) -> String { mem::take(&mut
self.name) }`, or `Option::take` or `mem::replace` from `&mut self`. The `rust-discipline` skill,
when it is installed, owns the guard design that avoids E0509: put `impl Drop` on a one-field
guard type, not on the aggregate.

## Match guards

A guard runs after the pattern matches and before the arm body. The bindings are shared borrows
while the guard runs. A by-value binding moves or copies only after the guard succeeds.

### A guard on an or-pattern can run more than once

The guard applies to the complete or-pattern. It runs once for each alternative that matches
until one succeeds. Do not put metrics, mutation, I/O, or one-shot work in it. Read
`references/guard-probes.md` when you need a runnable proof of the guard count, a guard
mutation error, or a guard lock hold.

If a guard needs expensive or stateful work, match first and run ordinary control flow in the
arm.

### Guard mutation

A guard can mutate a local that the pattern does not bind. rustc rejects two other mutations:

- Assigning to or mutably borrowing the scrutinee gives E0510 (`cannot assign ... in match
  guard`, `cannot mutably borrow ... in match guard`).
- Assignment through a pattern binding gives E0594. A `&mut` method call through it, such as
  `v.push(2)`, gives E0596. Both carry the note "variables bound in patterns are immutable until
  the end of the pattern guard". This applies to `ref mut` bindings too.

Move the mutation into the arm body, or compute a boolean before the match. When rustc rejects
a guard, keep the exact pattern in the probe. A rewrite into an `if` proves a different program.

### Guard temporaries

An `if let` guard keeps its scrutinee temporaries until the arm body ends, or drops them before
the next arm when the guard fails. A boolean guard (`if cond`) drops them as soon as the condition
is evaluated. A `lock()` in an `if let` guard therefore holds the lock through the arm body.

Copy the value out before the `match` to release the lock first.

An arm with any guard does not count toward exhaustiveness. This includes an `if let` guard
whose pattern cannot fail. rustc still reports E0004 for the value that the guard covers.

## Scrutinee place versus value

Matching a place inspects existing storage. Matching a value expression creates a temporary.

| Scrutinee | Ownership question |
| --- | --- |
| `match local` | Which fields move from or borrow the local? |
| `match &local` | Which bindings inherit a reference binding mode? |
| `match make()` | Does the temporary hold a lock or borrow through the arms? |
| `match *pointer` | Is the dereference a safe place, and what can move from it? |

Do not change a scrutinee between a place and a value (a named guard versus an inline `lock()`),
and do not rewrite `match` into `if let` or the reverse, without checking destructor and borrow
timing. The `rust-borrow-semantics` skill, when it is installed, owns when each scrutinee
temporary drops, on each edition.

## Or-patterns

Every alternative must bind the same names with the same types and binding modes. Prefer one arm
per case when alternatives need different guards or side effects. Use an or-pattern when the
alternatives have the same binding contract and the same body.

## Match exhaustiveness is a behavior decision

Do not add `_ => {}` only to satisfy rustc. A new variant then takes the wildcard path
silently. For a closed enum this crate owns, list every variant. The `rust-discipline` skill, when
it is installed, holds the exhaustiveness policy and the `wildcard_enum_match_arm` lint.

## Edition 2024 match ergonomics

Edition 2024 rejects a `mut`, `ref`, or `ref mut` modifier on a binding while the default
binding mode is `ref` or `ref mut`, and a `&` or `&mut` pattern unless the default binding mode
is move. The errors have no error code; do not invent one for a probe. Read
`references/edition-2024-patterns.md` when a `... within an implicitly-borrowing pattern` error
appears, or before you migrate a crate: it has the message-to-fix table, the edition 2021
meaning of `mut`, and the `rust_2024_incompatible_pat` migration steps.

Do not copy a pattern from an edition 2021 example into an edition 2024 crate without compiling
it in the target crate.

## Related skills

When installed: `rust-borrow-semantics` owns temporary scopes and two-phase borrows,
`rust-compiler-errors` owns general E0382, E0507, and E0499 triage, and `rust-callback-bounds`
owns closure patterns and higher-ranked callback bounds.

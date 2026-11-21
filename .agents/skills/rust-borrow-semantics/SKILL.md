---
description: Use when a Rust lock guard or temporary lives longer or shorter than the code suggests, such as a hang on a second lock() while a match scrutinee still holds the guard, or a borrow that depends on expression form (temporary lifetime extension, the drop scope of a temporary, two-phase borrows and autoref, place versus value). Includes edition 2024 if_let_rescope and tail_expr_drop_order changes, and E0716 there. Not for general borrow-checker triage (use rust-compiler-errors).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-borrow-semantics
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: f47937ec8c3ffce8ae502019cb5d5b404cf341fc
name: rust-borrow-semantics
---
# Rust Borrow Semantics

Keep the exact source expression until you know every borrow and drop point. A manual
desugaring, an added helper call, a macro, or an edition change can move those points while the
types stay the same.

## Start with four facts

Record these facts before you propose a fix:

1. Is each operand a place expression or a value expression?
2. Which expression creates each temporary?
3. Which borrow is explicit, and which borrow does autoref or reborrow insert?
4. Which edition and which minimum supported Rust version (MSRV) apply? Temporary scope and
   lifetime extension rules changed in several releases from 1.79 to 1.98.

Then write the expected reservation, activation, and drop points. Prove them with a probe (see
[Verification](#verification)).

## Triage

| Symptom | Likely cause | First action |
| --- | --- | --- |
| Method call compiles, UFCS rewrite fails with E0502 | The rewrite removed an implicit two-phase borrow | Restore the method call, or compute the other argument first |
| E0716 after you add a helper call | A function call does not extend the temporary | Name the owner in a `let` before the call |
| `Some(&temp())` or `Wrapper(&temp())` compiles on stable, fails with E0716 on the MSRV lane | Constructor lifetime extension needs Rust 1.89 | Name the owner, or raise the MSRV on purpose |
| New E0716 at a block tail after a move to edition 2024 | The tail temporary now drops at the end of the block | Move the block value into a named `let` |
| After a move to edition 2024, a guard releases earlier: a deadlock disappears, a race appears, or `try_lock` succeeds in `else` | The `if let` scrutinee or the tail temporary scope narrowed | Bind the guard to a named local in an explicit block |
| A lock stays held through a whole `match`, or a second `lock()` later in the same statement deadlocks | A `match` scrutinee temporary lives through every arm on every edition. It drops at the end of the enclosing statement, or before the block's locals when the `match` is a block tail on edition 2024 | Read the value out in a block that drops the guard (`let v = { let g = m.lock().expect("m mutex poisoned"); g.clone() };`), then match on `v`. For `if let` guards (1.95+), see the `rust-pattern-semantics` skill |
| E0502 or E0499 at a call to a function that returns `impl Trait`, only on edition 2024, with the note "may capture more lifetimes than intended" | The opaque return type captures the argument lifetime. This is a signature rule, not a temporary scope | Fix the callee signature, not the caller. See the `rust-compiler-errors` skill |
| E0716 or a moved drop point in a `pin!`, `format_args!`, `write!`, `writeln!`, or `assert_eq!` call after a rewrite, a toolchain change, or on the MSRV lane | Macro temporaries have their own rules, changed in 1.89-1.98 | Probe the exact macro call on the MSRV and on stable |
| Adding braces changes whether code compiles | The block changed a temporary or borrow scope | Mark the exact creation and drop points |
| A clone makes the error disappear | Ownership was duplicated, not explained | See [Clone is not a scope operator](#clone-is-not-a-scope-operator) |

## Place expressions and value expressions

A place expression identifies storage. A value expression produces a value. The difference
decides whether an operation moves from existing storage or creates a temporary.

| Shape | Usually a place | Usually a value |
| --- | --- | --- |
| Local, static, dereference, index, field | Yes | No |
| Literal, arithmetic result, function call, constructor | No | Yes |
| Parentheses | Keep the inner classification | Keep the inner classification |
| Block tail | Depends on the tail expression | Depends on the tail expression |

Do not classify from the type. A `String` can be a local place or the value that a call returns.

## Temporary scopes

A temporary drops at the end of its enclosing temporary scope. The important scopes are the
statement, a condition, a match arm, and a block tail.

Edition 2024 narrows two scopes. It drops `if let` scrutinee temporaries before the `else`
block, and it drops block-tail temporaries before the block's locals. Both changes release a
guard earlier, never later. A `match` scrutinee temporary still lives through every arm.

This probe runs on edition 2024. On edition 2021 the first assertion fails:

```rust,run
use std::sync::Mutex;

fn main() {
    let m = Mutex::new(None::<u32>);
    let free_in_else = if let Some(v) = *m.lock().unwrap() {
        v == 0
    } else {
        m.try_lock().is_ok()
    };
    assert!(free_in_else);
    let free_in_arm = match *m.lock().unwrap() {
        Some(v) => v == 0,
        None => m.try_lock().is_ok(),
    };
    assert!(!free_in_arm);
}
```

The narrowing can also reject code that compiled on edition 2021:

```rust,compile_fail,E0716
fn main() {
    let len = { &String::from("1234") }.len();
    println!("{len}");
}
```

When a resource guard must release before later work, bind it and make the drop point explicit
with braces. The drop point is then the same on every edition:

```rust
use std::sync::Mutex;

let state = Mutex::new(vec![1]);
let first = {
    let guard = state.lock().expect("state mutex poisoned");
    guard[0]
};
assert_eq!(first, 1);
assert!(state.try_lock().is_ok());
```

## Temporary lifetime extension

Some `let` patterns (`ref` bindings) and initializers extend a borrowed temporary to the end of
the block. The rule is syntactic. It is not general lifetime inference. Since Rust 1.89 the
extension also passes through the arguments of a tuple struct or tuple variant constructor. On an
older MSRV the two constructor lines below fail with E0716:

```rust,run
fn make() -> String { String::from("alive") }

struct Wrapper<'a>(&'a String);

fn main() {
    let value = &make();
    let some = Some(&make());
    let wrapped = Wrapper(&make());
    assert_eq!(value, "alive");
    assert_eq!(some.map(String::as_str), Some("alive"));
    assert_eq!(wrapped.0, "alive");
}
```

An ordinary function call does not extend the temporary:

```rust,compile_fail,E0716
fn make() -> String { String::from("short") }
fn keep(value: &String) -> &String { value }

fn main() {
    let value = keep(&make());
    println!("{value}");
}
```

Fix this shape. Name the owner before the borrow:

```rust
fn make() -> String { String::from("long") }
fn keep(value: &String) -> &String { value }

let owner = make();
let value = keep(&owner);
assert_eq!(value, "long");
```

## Two-phase borrows

A two-phase mutable borrow starts as a reservation and becomes exclusive at activation. Shared
access can occur between those points. Rust creates two-phase borrows only for these implicit
borrows:

| Implicit shape | Typical example |
| --- | --- |
| Autoref of a mutable method receiver | `values.push(values.len())` |
| Mutable reborrow in a function argument | Passing an existing `&mut T` to another call |
| Mutable borrow for overloaded compound assignment | `place += rhs` |

The standard method-call shape compiles:

```rust,run
fn main() {
    let mut values = vec![10, 20];
    values.push(values.len());
    assert_eq!(values, [10, 20, 2]);
}
```

An explicit `&mut` borrow is not two-phase. A manual UFCS rewrite can therefore fail even when
the method call compiles:

```rust,compile_fail,E0502
fn main() {
    let mut values = vec![10, 20];
    let receiver = &mut values;
    Vec::push(receiver, values.len());
}
```

Do not explain the first program as evaluation order alone. Record these points:

1. The receiver autoref reserves `&mut values`.
2. Rust evaluates the other arguments. Shared reads can occur during the reservation.
3. The call activates the mutable borrow.
4. The borrow ends after the call, unless the result keeps it alive.

Rust still rejects a conflicting write between reservation and activation.

## Clone is not a scope operator

A clone that makes a borrow error disappear duplicates ownership. It does not explain the
borrow. If the cause is a temporary scope, name the owner or add a block. If the cause is a
two-phase borrow, keep the method-call shape or compute the argument first. Clone only when an
independent copy matches the domain, for example a small value or shared read-only state. Do not
clone a resource handle or a unique buffer to escape a scope.

## Edition 2024 review

Run the two migration lints on the crate while it is still on edition 2021. They go quiet after
the edition changes, because the behavior has already changed:

```bash
cargo clippy --locked -p <crate> --all-targets --all-features -- -W if_let_rescope -W tail_expr_drop_order
```

The lints see only compiled code. Run the command once for each supported feature set and each
shipping target with cfg-gated code (`--target <triple>`).

Both lints are allow-by-default members of `rust_2024_compatibility`. Interpret the results:

- `cargo fix --edition` rewrites an `if_let_rescope` site into a `match`. The `match` keeps the
  edition 2021 lock hold, and any deadlock that came with it. Keep that hold only when the
  `else` branch must run while the lock is still held. Then prefer a named guard in an explicit
  block over the generated `match`, because the block shows the hold. Otherwise restore the
  `if let` and accept the earlier drop.
- `cargo fix --edition` reports `tail_expr_drop_order` sites but changes nothing. Inspect each
  site, and bind the temporary to a named local when its drop order matters.
- Do not use an edition migration as a lock-lifetime refactor. Run behavior tests for code whose
  temporaries own locks, transactions, file descriptors, or other guards.

The feature-set and target rule, the migration workflow, the staged per-crate order, and the edition
2024 change list live in the `cargo-workflows` skill; without it, run `cargo fix --edition` once
per supported feature set and once per shipping `--target`.

## Verification

Use the smallest proof that decides the question:

1. Put the exact expression in a standalone file. Do not simplify it first.
2. Compile the probe on the workspace edition
   (`rustc --edition <edition> --emit=metadata probe.rs`). When the fix relies on a rule that is
   newer than the MSRV, compile the probe with `rustc +<msrv>` and the real fix with
   `cargo +<msrv> check --locked`. Examples: `if`/`match` lifetime extension (1.79+), `use<..>`
   (1.82+; in a trait, 1.87+), constructor lifetime extension (1.89+), and macro temporaries
   (changed in 1.89-1.98).
3. Use a `compile_fail` probe for a rejected borrow, and name the expected error code.
4. Use a runtime assertion, for example `try_lock().is_ok()`, when the question is a drop point
   or a destructor order.
5. Run the real workspace tests after the probe. The probe proves the language rule, not the
   application invariant.

Do not use Miri to answer whether code type-checks; use it after compilation for unsafe
aliasing or provenance.

## Related skills

When installed: `rust-callback-bounds` owns closure inference and higher-ranked callable bounds,
`rust-variance` owns lifetime subtyping through type constructors, and `rust-unsafe` owns
aliasing and provenance after borrow checking (`cargo +nightly miri test --locked`).

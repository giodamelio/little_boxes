# Closure Capture Semantics

Deep material for [SKILL.md](../SKILL.md). Read it when `move`, a field projection, or a
refactor changes the closure call trait, lifetime, `Send` status, size, or cleanup timing. The
capture errors below were checked on rustc 1.99.1, edition 2024, on x86_64-unknown-linux-gnu.

- [`move` does not mean `FnOnce`](#move-does-not-mean-fnonce)
- [Capture precision depends on the projection](#capture-precision-depends-on-the-projection)
- [Rust 1.100 `if let` captures](#rust-1100-if-let-captures)
- [Do not depend on capture drop order](#do-not-depend-on-capture-drop-order)
- [Edition review](#edition-review)
- [Checklist](#checklist)
- [Authoritative references](#authoritative-references)

## `move` does not mean `FnOnce`

The `move` keyword selects how the closure captures from its environment. It does not select the
call trait. The closure body selects the strongest implemented call trait:

| Body use of a capture | Call trait consequence |
| --- | --- |
| Read through a shared borrow | Can implement `Fn` |
| Mutate through a unique or mutable borrow | Can implement `FnMut` |
| Move or consume the captured value | Implements only `FnOnce` |

```rust,run
fn call_twice(f: impl Fn()) {
    f();
    f();
}

fn main() {
    let text = String::from("captured by value");
    call_twice(move || assert!(!text.is_empty()));
}
```

Do not add `clone`, `Box<dyn FnOnce>`, or a one-shot wrapper because a closure says `move`.
Compile it against the smallest required bound.

## Capture precision depends on the projection

Edition 2021 and later can capture disjoint fields instead of the complete owner. The exact
projection still matters.

| Expression shape | Review risk |
| --- | --- |
| Named struct field | The closure can capture only that field |
| Array or slice index, sub-slice, or array pattern | The closure always captures the complete array or slice |
| Field of a packed struct | The closure captures a safe prefix or the complete packed value to avoid an unaligned reference |
| Dereference through `Box<T>` | A non-`move` closure that borrows the boxed field captures only that field. A `move` closure, or a by-value use of the field, captures the complete `Box`, and a later use of the `Box` fails with E0382 |
| Dereference through `Rc<T>` or custom `Deref` | The closure captures the smart pointer, not an invented interior place |
| Raw-pointer dereference | Unsafe access and capture ownership are separate proofs |
| `match`, `if let`, `let .. else`, or `matches!` on an enum with more than one variant | The discriminant read captures the place by shared borrow, even when the pattern binds nothing |
| Pattern on a `#[non_exhaustive]` enum, even with one variant | Shared-borrow capture. For an enum from another crate this was already so before Rust 1.95; for an enum in the same crate it is new in 1.95 |
| `let Ok(v) = r` or a `match` on `Result<T, Infallible>`, or on another enum whose other variants are uninhabited | Shared-borrow capture of the discriminant since Rust 1.94; 1.93 and earlier did not capture it |

A discriminant capture shows up as E0506, E0502, or E0505 when code assigns, mutably borrows, or
moves the enum while the closure is alive. Two rows are new, so a closure that compiled before can
fail after a toolchain bump: the uninhabited-variant row in 1.94, and the same-crate
`#[non_exhaustive]` row in 1.95:

```rust,compile_fail,E0506
#[non_exhaustive]
enum Reply { Ok(u32) }

fn main() {
    let mut reply = Reply::Ok(1);
    let check = || { let Reply::Ok(_) = reply; }; // reads the discriminant
    reply = Reply::Ok(2);                          // E0506 since Rust 1.95
    check();
}
```

End the closure's last use before the assignment, or compute the match result before the closure
and capture that value.

This can change whether the closure is `Send` or `'static`. Inspect the captured owner, not only
the field type visible inside the body.

Use a compile probe at the real bound:

```rust
fn require_send_static(_: impl Send + 'static) {}

let owner = make_owner();
require_send_static(move || use_field(&owner.field));
```

If the probe fails, do not write `unsafe impl Send`. Reshape ownership so the closure captures a
safe owner.

## Rust 1.100 `if let` captures

Rust 1.100 makes `if let` captures more precise. An irrefutable pattern can now
capture separate fields instead of the whole owner. A borrow of one field can
then prevent a closure from being `'static`, even when another field moves into it.
Recheck `Send`, lifetimes, and cleanup order after the bump. A local binding does
not guarantee that the closure owns the complete binding.

```rust,run
fn main() {
    let pair = (String::from("kept"), String::from("moved"));
    #[allow(irrefutable_let_patterns)]
    let consume = || {
        if let (_, value) = pair {
            assert_eq!(value, "moved");
        }
    };
    assert_eq!(pair.0, "kept");
    consume();
}
```

This probe checks field capture on edition 2024. It does not check a project's
`Send` bounds or destructor order. Compile a caller at the real bound for those claims.

## Do not depend on capture drop order

Locals, fields, and closure captures do not share one universal drop order. Precise by-value
captures can also cause parts of one owner to drop at different times. Do not encode lock order,
transaction order, or foreign-resource teardown in closure capture order.

When order matters:

1. Put the resources in one named owner.
2. Implement an explicit `close` or consuming finish method.
3. Keep `Drop` as an infallible fallback.
4. Test the visible cleanup order with a small event log.

## Edition review

A closure created in an edition 2018 crate can capture more of an owner than the same source in
an edition 2021 or 2024 crate. During the 2018 to 2021 migration, run `cargo fix --edition`. It
turns on the allow-by-default `rust_2021_incompatible_closure_captures` lint (part of
`rust_2021_compatibility`) and inserts `let _ = &owner;` where the new capture changes drop order
or an auto trait. For a report with no edits, set `rust_2021_incompatible_closure_captures = "warn"` in the `[lints.rust]`
table and run `cargo check`. Do not pass the lint through `RUSTFLAGS` or
`CARGO_ENCODED_RUSTFLAGS`: either variable replaces every config-file rustflags value. Then recheck
closure size, `Send`, lifetime, and destructor-timing assertions. The `cargo-workflows` skill owns
the edition-migration workflow, when it is installed: one crate at a time, with
`cargo fix --edition --locked -p <crate> --all-features`.

## Checklist

- The required callable trait comes from actual body use, not from `move`.
- The exact captured place is known for every field, index, dereference, and packed access.
- `Send` and `'static` are checked against the captured owner.
- Cleanup correctness does not depend on capture drop order.
- Edition migration tests ownership and destructor timing.

## Authoritative references

- [Rust Reference closure expressions](https://doc.rust-lang.org/reference/expressions/closure-expr.html)
- [Rust Reference closure capture modes](https://doc.rust-lang.org/reference/types/closure.html#capture-modes)
- [Rust Reference call traits and coercions](https://doc.rust-lang.org/reference/types/closure.html#call-traits-and-coercions)

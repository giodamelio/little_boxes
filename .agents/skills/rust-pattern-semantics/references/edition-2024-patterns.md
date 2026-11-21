# Edition 2024 patterns and version-gate errors

## Version-gate error messages

| Feature | Needs | Error when the gate is not met |
| --- | --- | --- |
| Let chains in `if` and `while` (`if let A = a && cond`) | Edition 2024 and rustc 1.88+ | Edition 2021: `let chains are only allowed in Rust 2024 or later` (no error code). Edition 2024 on rustc 1.85-1.87: E0658 `` `let` expressions in this position are unstable `` (measured on rustc 1.87.0) |
| `if let` guards, and a `let` chain inside a guard (`if let P = e && cond`) | rustc 1.95+, every edition | E0658 `` `if let` guards are experimental `` (measured on rustc 1.94.0) |
| `std::assert_matches!` | rustc 1.96+ | E0658 `` use of unstable library feature `assert_matches` `` on rustc 1.95, and for the `use` form on older toolchains. E0433 or `cannot find macro` for the path form on rustc 1.94 and older (measured on rustc 1.87.0-1.95.0) |
| Match ergonomics reservations | Edition 2024 | Uncoded errors, see [Match ergonomics reservations](#match-ergonomics-reservations) |

## Match ergonomics reservations

Each message and its fix:

| Message | Example on `value: &Option<T>` | Fix |
| --- | --- | --- |
| `cannot explicitly borrow within an implicitly-borrowing pattern` | `Some(ref x)` | Remove `ref`. `x` already borrows. On `&mut Option<T>`, write `&mut Some(ref x)` to keep a shared borrow (rustc's help) |
| `cannot mutably bind by value within an implicitly-borrowing pattern` | `Some(mut n)` | For a `Copy` type, write `&Some(mut n)`. Otherwise remove `mut` and clone into a new `let mut` in the body |
| `cannot explicitly dereference within an implicitly-borrowing pattern` | `Some(&n)` on `&Option<&u32>` | Write `&Some(&n)` |

In edition 2021, `mut` on a binding resets the default binding mode to move. `Some(mut n)` on
`&Option<u32>` compiles there and makes `n` a `u32` copy, not a `&u32`. On `&Option<String>` it
fails with E0507.

To migrate:

1. List the affected patterns with
   `cargo clippy --locked --all-targets --all-features -- -W rust_2024_incompatible_pat`. Use
   `--all-features` only when the features are additive. Otherwise run it once per supported
   feature set. Repeat it once per shipping `--target <triple>`, as the `cargo-workflows` skill
   describes when it is installed.
2. `cargo fix --edition` applies this lint, which is part of `rust_2024_compatibility`. It
   rewrites each pattern to a fully explicit form with the same meaning in every edition.
3. Review each rewrite. The explicit form keeps the 2021 meaning, including a silent copy. Decide
   per site whether a borrow was intended.

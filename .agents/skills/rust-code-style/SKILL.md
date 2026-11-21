---
description: Use when laying out Rust modules and source files, choosing visibility (pub, pub(crate), pub(super)), writing lib.rs re-exports or a prelude, ordering items, fixing import grouping, choosing thiserror or anyhow, writing rustdoc Errors and Panics sections, or reviewing layout and readability. Triggers on mod.rs, re-export, unreachable_pub, Iterator for_each, or flat_map over a Result.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-code-style
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 88ccf8f338302b95a2834a7a84c24db873625fe5
name: rust-code-style
---
# Rust Code Style

This skill holds the layout and readability decisions that `rustfmt` cannot make. `rustfmt`
owns line width, brace placement, wrapping, and the sort order inside each contiguous block of
`mod` declarations or `use` imports. The `rust-lints` skill owns `clippy.toml`, `rustfmt.toml`,
and the workspace lint tables. The `rust-crate-architecture` skill owns crate boundaries and
dependency direction.

Most rules here are catalog defaults, not Rust facts. Each one carries its reason, so a team can
override it on purpose. The `flat_map` and `lines()` rules are exceptions: they prevent lost
errors and hangs, so they hold everywhere.

## Checks

Use the tool where one exists. Review the rest by hand with the [checklist](#review-checklist).

| Rule | Check |
| --- | --- |
| Formatting, and the order inside each `mod` or `use` block | `cargo fmt --all --check` |
| No `pub` item that the crate root cannot reach | rustc `unreachable_pub` (allow by default; set it to `warn` in the workspace lint table) |
| An inline `#[cfg(test)] mod tests` is the last item | `clippy::items_after_test_module` (warn by default; needs the test cfg, as in `cargo clippy --all-targets`) |
| `# Errors` and `# Panics` doc sections | `clippy::missing_errors_doc`, `clippy::missing_panics_doc` (pedantic) |
| `# Safety` doc section | `clippy::missing_safety_doc` (warn by default) |
| Broken intra-doc links, public docs that link private items | The `cargo doc` gate in the `rust-lints` skill. It passes `--document-private-items`, so it also checks the links in private docs |
| Doc examples compile and pass | `cargo test --locked --workspace --doc` (library targets only) |
| No `Iterator::for_each` | `disallowed-methods` in `clippy.toml` |
| No `filter_map(Result::ok)` or `flatten()` on `io::Lines` | `clippy::lines_filter_map_ok` (warn by default) |

No tool checks import groups, the rest of the item order, caller-before-callee order, the depth
exception for `mod.rs`, or the early-return rule. A green run proves nothing about them.

## Module layout

Use `name.rs` next to a `name/` directory for every module that has children, so every editor
tab has a distinct name. Use `mod.rs` only three levels deep or more, where a module that lives
wholly in its directory then moves as one unit. Do not mix the two patterns at one directory
level: a reader then cannot predict where a module root is.

Default: `lib.rs` declares the modules first, then re-exports the public API item by item. A
glob re-export makes every new `pub` item public API with no review. A `pub mod prelude` is the
one module a caller may glob-import; build it from explicit `pub use` items. Avoid `#[path]`.

Read [references/module-layout.md](references/module-layout.md) when you create `lib.rs` or a
prelude, when a downstream crate reports E0659, E0034, or "`X` is ambiguous" after a prelude
change, when you enforce the layout with a lint, or when you include `build.rs` output.

## Visibility

| Level | Use it for |
| --- | --- |
| private (default) | An implementation detail of one module |
| `pub(super)` | A helper that only the parent module calls |
| `pub(crate)` | An item that other modules of the same crate use |
| `pub` | The public API, re-exported from `lib.rs` |

Start private. Widen one step only when a concrete caller in another module needs the item, and
narrow it when the caller goes away. Check `pub(super)` callers again when you move the module.
Do not write `pub(in path)` without a written reason: a module move breaks the build. Fix an
`unreachable_pub` finding; do not suppress it.

Do not widen visibility for a test: a child `mod tests` already reads its parent's private items.
A test that needs an item from another module belongs in the module that owns the item, or the
item is `pub(crate)` API.

## File structure order

Inside a `.rs` file, place the items in this order:

1. Crate and module attributes (`#![forbid(unsafe_code)]`,
   `#![doc = include_str!("../README.md")]`).
2. `mod` declarations: private first, then `pub mod`, with a blank line between the two groups.
   The `#[cfg(test)] mod tests` declaration goes at the bottom (step 10).
3. `use` imports (see [Import style](#import-style)).
4. `pub use` re-exports.
5. Constants and statics.
6. Type definitions: structs, enums, type aliases.
7. Trait definitions.
8. `impl` blocks: inherent first, then trait impls.
9. Free functions: public first, then private helpers.
10. `#[cfg(test)] mod tests`.

Put a blank line between steps 3 and 4 too. rustfmt sorts each contiguous block of `mod` or
`use` lines by name and ignores visibility, so without the blank lines `cargo fmt` mixes `mod`
with `pub mod` and moves `pub use crate::...` above `use std::...` (rustfmt 1.9.0). Inside each
of steps 5 to 9, put public items before private items, so the API surface reads first.

## Import style

Separate the imports into four groups, with one blank line between the groups:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use app_protocol::FrameKind;
use app_telemetry::SpanId;

use crate::types::SharedState;
use crate::util::format_duration;
```

The order is: `std` (and `core`, `alloc`), then external crates, then workspace crates, then
`crate::`, `self::`, and `super::`.

Stable `rustfmt` sorts inside a group but does not move an import between groups
(`group_imports` is unstable), so keep the groups by hand. Style edition 2024 sorts `u8` before
`u16`, and editions 2015 to 2021 the other way, so a direct `rustfmt` call can disagree with
`cargo fmt`; the `rust-lints` skill owns `style_edition` in `rustfmt.toml`.

Use one braced import for two or more items of one module (`imports_granularity` is unstable).
Import the items the file uses often; write the full path for an item used once, such as
`std::time::Instant::now()`.

## Function rules

### Caller before callee

Place a calling function before the functions it calls, so the file reads top to bottom.

### Iterator chains and collectors

Keep every closure in a `.map()`, `.filter()`, or `.collect()` chain pure. Write a side effect
as a `for` loop.

Ban `Iterator::for_each` through `disallowed-methods` in `clippy.toml` (the `rust-lints` skill
holds the entry). This is a catalog default: a `for` loop shows the effect in plain sight.

Never `flat_map` or `flatten` over a `Result`. `Err` yields zero items, so the failure
disappears, and no lint or type error reports it. Use `collect` into a `Result` to stop at the
first error, or `partition` to keep both halves. Use `filter_map` with `.ok()` only when the drop
is the intent.

Never skip the errors of `BufRead::lines()` with `filter_map`, `flat_map`, or `flatten`: a reader
that fails on every call yields `Err` forever, and the loop hangs. Collect into
`io::Result<Vec<String>>`, or use `map_while(Result::ok)` when a silent stop is the intent.

Read [references/iterator-style.md](references/iterator-style.md) when you judge whether a
closure hides a side effect, choose a collector for fallible items, weigh
`clippy::needless_for_each`, or see a benchmark that argues for `for_each`.

### Business paths use `if`/`else` and `match`

Keep a guard return (`return`, `let ... else`) for bookkeeping: a null, handle, or permission
check. `?` is not limited. Write mutually exclusive business paths as `if`/`else` or `match`, a
catalog default: one `match` shows every alternative in one place.

## Error handling

The `rust-error-handling` skill owns error-type design, including `thiserror` versus `anyhow`;
this skill owns only the rustdoc `# Errors` section below.

## Rustdoc contract

A doc comment on a public item is part of the API. Three sections carry what the signature
cannot:

| Section | Required on | States |
| --- | --- | --- |
| `# Errors` | Every public `fn` that returns `Result` | Which variants occur, and what causes each one |
| `# Panics` | Every public `fn` that can panic | The exact condition. Write "Never" when a reader would expect a panic |
| `# Safety` | Every public `unsafe fn` | What the caller must guarantee |

Turn on `missing_errors_doc` and `missing_panics_doc` for a published crate. Read
[references/rustdoc.md](references/rustdoc.md) when you write a doc comment or a doc example.

### Link with intra-doc links, not URLs

Write ``[`ConfigError::NotFound`]`` and let rustdoc resolve it: the `cargo doc` gate then
reports a stale link, while a hand-written docs.rs URL pins a version and breaks silently.

### Doc examples are tests

`cargo test` compiles and runs every doc example in a library target; a binary target runs none.
Use `ignore` only when the example cannot compile at all: nothing then reports it when it goes
stale.

## Naming

- The rustc lints `non_snake_case`, `non_camel_case_types`, and `non_upper_case_globals` enforce
  the case rules. Do not suppress them, except on names that a foreign ABI fixes (bindgen
  output, a hand-typed `Java_*` export on jni 0.21). Suppress those on the smallest item, with a
  reason (the `rust-lints` skill). On jni 0.22, `#[jni_mangle]` generates the `Java_*` symbol
  from a snake_case function (the `rust-jni` skill).
- Name a test after the behavior it proves: `fn decoder_rejects_truncated_frame()`.
- API method naming: the `rust-discipline` skill, when it is installed.
- Write a crate name with hyphens in `Cargo.toml` (`app-protocol`). The path in code then uses
  underscores (`app_protocol`).
- Give the crates of one workspace one shared prefix, so an import shows its origin.

## Review checklist

Use this list when you review a diff for layout and readability.

- [ ] A new module with children uses `name.rs` plus `name/`, not `mod.rs`.
- [ ] `lib.rs` declares the modules first, then re-exports item by item, with no glob.
- [ ] A prelude uses explicit `pub use` items.
- [ ] Every new item has the narrowest visibility that its callers need. `unreachable_pub` is
      clean.
- [ ] No `pub(in path)` without a written reason.
- [ ] Each file follows the ten-step order, with public items before private ones in steps 5 to
      9, and a blank line after the private `mod` block and before the `pub use` block.
- [ ] The imports are in four groups, separated by blank lines.
- [ ] A caller function comes before the functions it calls.
- [ ] No closure in an iterator chain has a side effect, and no `for_each` appears.
- [ ] No `flat_map` or `flatten` over a `Result`, and no `filter_map`, `flat_map`, or `flatten`
      on `lines()`.
- [ ] Business paths use `if`/`else` or `match`, not a chain of early returns.
- [ ] No `.unwrap()` outside tests and examples. Each `.expect()` states a real invariant.
- [ ] A crate with no hand-written `unsafe` carries `#![forbid(unsafe_code)]`.
- [ ] Each public `Result`, panicking, or `unsafe` function has its rustdoc section.

The `rust-tdd` skill owns test placement and the pinned `expected` on each `#[should_panic]`;
`rust-unsafe` owns `# Safety` contracts; and `rust-panic-safety` owns `catch_unwind` policy,
each when it is installed.

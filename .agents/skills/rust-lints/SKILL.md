---
description: Use when editing workspace.lints, clippy.toml, or rustfmt.toml, choosing a rustc, clippy, or rustdoc lint level, rolling out a stricter lint, reviewing an allow or expect suppression, or triaging a red clippy, cargo fmt, or cargo doc gate. Triggers on [lints] workspace = true, -D warnings, CARGO_BUILD_WARNINGS, unfulfilled_lint_expectations, disallowed-methods, and clippy msrv. Not for deny.toml license, advisory, or ban policy; use rust-security.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-lints
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 0a4e066d797cf8925f52cd7d06b0ad93eee3979d
name: rust-lints
---
# Rust lints

A written rule ("no unsafe", "no bare allow") is not enforcement until a lint level or a CI
check carries it. Document only the level that is deployed: a document that shows a target
level as the current one makes reviewers stop checking. Read
[references/triage.md](references/triage.md) when a clippy, `cargo fmt`, or `cargo doc` gate is
red.

## Where lint policy lives

Put every lint level in the workspace root `Cargo.toml`. Each member carries only:

```toml
[lints]
workspace = true
```

A per-crate level in a member manifest is invisible to a reader of the root policy, and it
drifts. The root `Cargo.toml` owns `[workspace.lints.rust]`, `[workspace.lints.clippy]`,
`[workspace.lints.rustdoc]`, and `rust-version` in `[workspace.package]`, which clippy reads as
the MSRV. `clippy.toml` beside the root manifest owns thresholds, disallowed paths, and test
exemptions; `rustfmt.toml` owns formatting.

A crate attribute overrides the inherited level in both directions: `#![warn(lint)]` in
`lib.rs` lowers a workspace `deny` (1.99.1). Only `forbid` cannot be lowered. Restate an
inherited lint in source only at the same or a stricter level.

Read [references/gate-details.md](references/gate-details.md#cargo-lints-on-rust-1100)
when you set Cargo 1.100+ lint policy or triage `cargo::unused_dependencies`.

## Verification

While you iterate, run `cargo clippy --locked -p <crate> --all-targets -- -D warnings`. Before a
commit or a merge, run the workspace gate:

```bash
# --all-features only when the features are additive; see the first bullet below.
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items --all-features
cargo nextest run --locked --workspace --all-features  # without cargo-nextest: cargo test
cargo test --locked --workspace --all-features --doc   # nextest does not run doctests
cargo deny --config deny.toml --locked check  # when dependencies, Cargo.lock, or deny.toml change
```

Read [references/gate-details.md](references/gate-details.md) for the reason behind each rule
below, and when you choose feature sets, targets, or `CARGO_BUILD_WARNINGS`:

- `--all-features` is correct only for additive features; otherwise lint each supported set.
- Clippy does not run rustdoc lints. Only the `cargo doc` line catches a broken intra-doc link.
- A host run does not lint target-`cfg` code: add `--target <triple>` per shipped target.
- `-- -D warnings` does not deny `linker_messages`; `CARGO_BUILD_WARNINGS=deny` (1.97+) does.
- Do not use `RUSTFLAGS=-Dwarnings`: it rebuilds every dependency and replaces config rustflags.
- A toolchain bump adds lints to `pedantic` and `nursery`. Pin the toolchain.
- When `cargo fmt --all -- --check` is red before your change, format only the changed files
  with `rustfmt --edition 2024 --config skip_children=true <file>`.

After a lint change, audit suppressions. Every hit needs `reason = "..."`, and every
`// TODO(<owner>)` needs a tracked follow-up. Each crate root that the third command lists must
own `unsafe`:

```bash
rg '#!?\[(allow|expect)\(' --type rust -n
rg 'cfg_attr\([^)]*(allow|expect)\(' --type rust -n
rg --files-without-match 'forbid\(unsafe_code\)' -g '**/src/{lib,main}.rs'
# without rg:
grep -rL --include=lib.rs --include=main.rs --exclude-dir=target 'forbid(unsafe_code)' .
```

## The `[workspace.lints]` table

Read [references/workspace-lints.md](references/workspace-lints.md) when you create or change
the table: it has the strict template for a new workspace and the pragmatic baseline for one
with a backlog. Read [references/lint-catalog.md](references/lint-catalog.md) when you justify a
level, pick `restriction` lints, or add the pointer/FFI or async block.

## `clippy.toml`

Read [references/gate-details.md](references/gate-details.md#clippytoml-behavior) for the reason
behind each key, and when an entry does not fire or a test exemption does not apply.

```toml
# No msrv key: clippy reads `rust-version`, and an msrv key that differs overrides it.

# Name both consumers of a duplicate major version in a comment.
allowed-duplicate-crates = ["bitflags"]

# Keep `true` for a crate on crates.io or a stable FFI or binding surface.
avoid-breaking-exported-api = true

# Non-test code uses `?`, or `.expect("<invariant>")` for a real invariant.
allow-unwrap-in-tests = true
allow-expect-in-tests = true
# Add the matching key only for a lint you enabled:
# allow-panic-in-tests            = true
# allow-print-in-tests            = true
# allow-dbg-in-tests              = true
# allow-indexing-slicing-in-tests = true

# ".." keeps clippy's default list and appends yours.
doc-valid-idents = ["..", "UniFFI"]

stack-size-threshold         = 4096   # large_stack_frames: a whole frame over 4 KiB (default 512000)
future-size-threshold        = 16384  # default; large_futures fires above 16 KiB
enum-variant-size-threshold  = 200    # default; large_enum_variant compares the two largest variants
large-error-threshold        = 128    # default; result_large_err fires when Err reaches it
# large-error-ignored        = ["your_crate::RareBigError"]
type-complexity-threshold    = 250    # default; a higher value is debt
too-many-arguments-threshold = 6      # default 7; skips extern fns, see references/triage.md

# The diagnostic prints the reason: write it for the person who hits the error.
disallowed-methods = [
  { path = "std::mem::forget",  reason = "use ManuallyDrop and document the Drop semantics" },
  { path = "std::env::set_var", reason = "not thread-safe; set it before threads start" },
  { path = "std::ptr::read",    reason = "use ptr::read_unaligned for byte buffers from I/O or FFI" },
  # Catalog default, not a Rust rule (the `rust-code-style` skill).
  { path = "std::iter::Iterator::for_each", reason = "use a `for` loop for side effects" },
]
```

Keep one `clippy.toml`, beside the root manifest: a member's own file replaces it, and every
root key stops applying to that member. Run `cargo clippy --locked --workspace --all-targets`
after you add a `disallowed-*` entry, because clippy can ignore a path it cannot resolve. Do not
add a per-test `expect` that an `allow-*-in-tests` key covers: it is unfulfilled.

## `rustfmt.toml`

Use stable options only. A stable toolchain ignores an unstable option with a warning, so
`cargo fmt --check` gives different results on nightly and stable.

```toml
edition = "2024"
style_edition = "2024"
max_width = 120
use_small_heuristics = "Max"
```

Set `edition` and `style_edition` to the steady-state edition, so that `cargo fmt` and a direct
`rustfmt` call sort imports the same way. Read
[references/gate-details.md](references/gate-details.md#rustfmttoml-and-the-edition) during an
edition migration or when `cargo fmt --check` fails on a file that looks formatted.

## `deny.toml`

`cargo deny --config deny.toml --locked check` (cargo-deny 0.20.0 or later) is the supply-chain
part of the gate, and the `rust-security` skill owns its policy and triage.

## Crate-level attributes

Put `#![forbid(unsafe_code)]` in every crate that has no hand-written `unsafe`, including a
crate that calls C through a safe wrapper crate. Set it per crate, never in
`[workspace.lints.rust]`: the one crate that owns `unsafe` cannot lower a workspace `forbid`.

```rust
#![forbid(unsafe_code)]
```

An export attribute (`#[unsafe(no_mangle)]`, `#[unsafe(export_name)]`) is unsafe code. A crate
that owns hand-written `unsafe` drops `forbid(unsafe_code)` and restates the unsafe lint floor
at its crate root; the `rust-unsafe` skill owns that list. Keep such crates few: they are your
audit surface.

A dependency's proc macro silences almost every lint on the code it emits, `unsafe_code`
included, so `forbid(unsafe_code)` does not prove a binding crate free of `unsafe`. Read
[references/lint-catalog.md](references/lint-catalog.md#lints-on-proc-macro-output) when a crate
uses a binding or FFI proc macro. Never suppress `not_unsafe_ptr_arg_deref`: safe code could then
pass a bad pointer. Make the export `unsafe` with a `# Safety` section
([binding-layer lints](references/lint-catalog.md#binding-layer-lints)).

## Suppressions

Order of preference:

1. **Fix the code.** A lint that fires is usually right.
2. **`#[expect(lint, reason = "...")]` on the smallest item that covers the violation.** A stale
   `expect` warns (`unfulfilled_lint_expectations`), so the suppression removes itself. Use the
   inner `#![expect(...)]` only for macro-expanded code that no item encloses, and for a
   test-helper module such as `tests/common/mod.rs` or an `examples/` file that calls
   `.unwrap()`.
3. **`#[cfg_attr(<cfg>, expect(lint, reason = "..."))]`** when the lint fires only on some
   targets or feature sets. A plain `expect` is unfulfilled on the other builds.
4. **`#[allow(lint, reason = "...")]`** only when no cfg describes where the lint fires.
5. **Never suppress workspace-wide.** Remove a lint that is noise everywhere, and record why. Do
   not lower a global level to silence one site.

`clippy::allow_attributes` and `clippy::allow_attributes_without_reason` enforce this order;
they miss an inner `#![allow]` (1.99.1), which the audit above finds. To grandfather a backlog,
put a per-site `expect` with a `// TODO(<owner>): fix` comment on each site.

## Add or tighten a lint

Use this loop for a new lint, a level promotion, and a threshold step. A new member takes
`[lints] workspace = true` and the [crate-level attributes](#crate-level-attributes); the
`rust-crate-architecture` skill owns the other steps.

1. Set the proposed level and run `cargo clippy --locked --workspace --all-targets` to list
   every violation. Do not write a baseline file.
2. Sort the findings by lint name into real defect, refactor, or noise.
3. Fix the defects. For a refactor too large for one change, add a per-site `expect` with a
   `// TODO(<owner>): fix`. Handle noise by [Suppressions](#suppressions).
4. Land the source fixes with the policy change or before it. Never weaken an unrelated lint to
   pay for this one.
5. Update the documented policy in the same commit.
6. Run the workspace gate and the tests: a lint fix that changes behavior builds clean.

Read [references/workspace-lints.md](references/workspace-lints.md#escalation-ladder) for the
next step up, such as `unwrap_used` to `deny` or a lower threshold. The `rust-discipline` skill
owns the `unwrap` and `expect` policy, `rust-panic-safety` the panic lints on an FFI path, and
`rust-hot-path` the fixes the performance lints ask for, when each is installed.

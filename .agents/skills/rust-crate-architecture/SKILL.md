---
description: Use when deciding Rust workspace crate boundaries and dependency direction, such as adding, splitting, merging, renaming, or removing a workspace crate, defining or enforcing dependency layers, fixing a layering violation or a dependency cycle between workspace crates, or choosing between a new crate and a module for new code or for a crate that grew too large. Not for module file layout or visibility (rust-code-style) or a cyclic package dependency through a proc-macro crate (rust-macros).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-crate-architecture
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 7cc4cfd513e8a245e354ea821321cbee7b6d1307
name: rust-crate-architecture
---
# Rust Crate Architecture

This skill decides which crates exist, which crate may depend on which, and whether new code
needs a crate or a module. The `cargo-workflows` skill owns manifests, dependency inheritance,
features, lockfile policy, and build commands. The `rust-code-style` skill owns module file
layout, the `lib.rs` re-export policy, and visibility.

## Read the graph from cargo

A workspace changes faster than its documentation. Read the current graph from cargo, not from
a diagram. Run each command from the workspace root, or pass
`--manifest-path <workspace-root>/Cargo.toml`.

```bash
# Members, manifest paths, versions, features, and targets.
cargo metadata --locked --no-deps --format-version 1

# Every edge between path crates, with its kind. It includes optional and
# target-gated edges, so it is the complete list to compare with the layer table.
cargo metadata --locked --no-deps --format-version 1 \
  | jq -r '.packages[] | .name as $n | .dependencies[] | select(.path != null)
           | "\($n) -> \(.name) [\(.kind // "normal")]"' | sort

# Forward dependencies of one crate: what it is allowed to know.
cargo tree --locked -p <crate> -e normal,build --target all

# Reverse dependencies: who knows the crate. Keep --workspace. Inside a member
# directory, cargo tree -i otherwise searches only that member's own tree.
cargo tree --locked --workspace -i <crate> --target all
```

[dependency-direction.md](references/dependency-direction.md#1-build-the-inventory) has the
edge list without jq. `cargo tree` resolves for the host target unless you pass
`--target all`, so an Android-only edge is invisible in a plain run, and it shows only the
features that the command line enables.

Treat an architecture document as stale until `cargo metadata` agrees with it, and fix it in the
change that moves a crate. Derive crate counts from `cargo metadata`; never hardcode them.

## The layer model

Give every crate exactly one layer. The layer says what the crate is allowed to know.

| Layer | Contents | May depend on | Must not depend on |
|-------|----------|---------------|--------------------|
| 0 Foundation | Pure data types, error types, format and protocol primitives. No I/O, no global state. | Third-party leaf crates, other Layer 0 crates | Layers 1-3 |
| 1 Domain | Algorithms, state machines, policy, decisions. Deterministic and testable without I/O. | Layer 0, other Layer 1 crates | Layers 2 and 3 |
| 2 Runtime | I/O, sockets, files, timers, task scheduling, process state. | Layers 0-1, other Layer 2 crates | Layer 3 |
| 3 Adapter | FFI boundary crates (`cdylib` / `staticlib`), platform bindings, CLI binaries. | Layers 0-2 | Other adapters. Nothing in the workspace depends on an adapter |

Place a new crate at the lowest layer that satisfies its dependencies. An adapter crate is a
sink: if something needs to depend on an adapter, the thing it needs is in the wrong crate.

## Verification

While you iterate, check the changed crate:

```bash
cargo tree --locked -p <changed-crate> -e normal,build --target all   # no higher-layer crate
cargo clippy --locked -p <changed-crate> --all-targets -- -D warnings
```

Once before commit, run the workspace gate:

```bash
cargo metadata --locked --no-deps --format-version 1     # inventory changed as intended
cargo tree --locked --workspace -i <foundation-crate> --target all   # who now depends on the shared crate
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny --config deny.toml --locked check     # cargo-deny 0.20.0 or later
```

Then compare the edge list with the layer table. Every `[normal]` and `[build]` edge must point
to a lower layer, or to the same layer below Layer 3. No such edge may point to an adapter. A
`[dev]` edge may point up. A green gate does not prove the layering: clippy and a default
cargo-deny policy accept every acyclic upward edge, and only the edge-list comparison checks
direction.

Checklist:

- [ ] The new or moved crate sits at the lowest layer that compiles.
- [ ] No `[normal]` or `[build]` edge points up, and no cross-stack edge appeared.
- [ ] Test-support crates are under `[dev-dependencies]` only.
- [ ] The manifest inherits the workspace keys, and `[lints] workspace = true` when the
      workspace defines `[workspace.lints]`.
- [ ] The crate has a `[workspace.dependencies]` entry.
- [ ] `#![forbid(unsafe_code)]` is present, or the `unsafe` need is stated.
- [ ] `Cargo.lock` is committed with the manifest change that caused it, and its diff holds only
      that change.
- [ ] The architecture document matches `cargo metadata` again.

## Triage

| Symptom | Cause | Fix |
|---------|-------|-----|
| "cyclic package dependency: package `<name> v<version> (<path>)` depends on itself" | A normal or build edge closes a cycle | Read the `Cycle:` lines to find the back edge. Extract the shared type into a lower-layer crate; delete the back edge |
| A Layer 1 crate has a Layer 2 crate in its tree | Upward dependency | Move the type down, or invert with a trait ([dependency-direction.md](references/dependency-direction.md), Fix A and Fix B) |
| E0116 or E0117 after a split or a type move | An inherent impl or an impl of a foreign trait stayed in the old crate | Move the impl with the type. An impl of a trait that the old crate defines may stay |
| A fixture crate appears in `cargo tree -e normal` | Test-support crate under `[dependencies]` | Move it to `[dev-dependencies]` |
| E0308 "expected `T`, found `my_crate::T`" in a unit test, with "multiple different versions of crate" | Dev-dependency cycle through a test-support crate | Move the test under `tests/` |
| "cannot update the lock file ... because --locked was passed" right after `cargo new` | `Cargo.lock` does not list the new member | Run `cargo metadata --format-version 1 > /dev/null` once |
| "error inheriting `<name>` from workspace root manifest's `workspace.dependencies.<name>`" | Member added without a `[workspace.dependencies]` entry | Add the entry |
| "failed to select a version for the requirement" on a path dependency | `version` in `[workspace.dependencies]` does not match the member | Align the version, or drop it when the policy allows |
| A member manifest lacks `[lints] workspace = true` or sets its own `version` | Hand-written manifest, keys not inherited | Add the `[lints]` table; use `version.workspace = true` |
| An upward edge exists only under `[target.'cfg(...)'.dependencies]` | Target gate hides a layering violation | Restructure; the gate does not make the edge legal |
| A crate that holds one type | No crate reason | Make it a module |

## Direction rules

Read [references/direction-rules.md](references/direction-rules.md) for the example and the
background of a rule.

1. **No upward dependencies.** Cargo accepts an upward edge that closes no cycle, and it is still
   a defect: it drags I/O, platform code, and heavy trees into crates meant to stay pure. Move
   the needed type down instead.
2. **Independent stacks stay independent.** Two product areas are two dependency trees that meet
   only at shared Layer 0 or Layer 1 crates that neither stack owns. A type both stacks need
   belongs in a Layer 0 crate that both already depend on.
3. **Test-support crates are dev-dependencies only.** A platform-support crate that also carries
   real runtime helpers may be a normal dependency of adapter crates only. A unit test in `src/`
   that uses a support crate which depends back on the crate under test fails with E0308; put
   that test under `tests/`.
4. **Observation is an adapter edge.** A diagnostics or metrics crate may depend on domain and
   runtime crates; they must not depend back on it. To emit an observation, define the sink as a
   trait or handle in a Layer 0 contracts crate that the monitor crate implements.
5. **Platform isolation uses `cfg`, not layer breaks.** A target-gated dependency is still a
   dependency. An edge that is illegal on one target is a violation on every target. For a
   multi-branch platform choice in a crate, use `cfg_select!` (MSRV 1.95), not the `cfg-if`
   crate.
6. **Adapter crates stay thin.** An FFI or platform crate marshals arguments, manages handle
   lifetimes, translates errors, and contains panics. Decisions belong one layer down. Generate
   each export with `native_method!` or `#[uniffi::export]`, and map each error to a stable
   `code()` string; the `rust-jni`, `uniffi-boundary`, and `ffi-error-progress-cancel` skills
   own the boundary.

## Crate or module?

A new crate costs a manifest, a `[workspace.dependencies]` entry, a `Cargo.lock` entry, a public
API for every item a dependent uses (the `rust-discipline` skill), and one more unit to locate.
Default to a module inside an existing crate. Create a crate when at least one of these is true:

- Two otherwise-unrelated crates need the code, and neither should inherit the other's tree.
- The code needs a different unsafe policy: the surrounding crate is `#![forbid(unsafe_code)]`
  and this code cannot be.
- The code needs a different dependency, feature, or target set: a host-only tool, a platform
  binding, a heavy optional backend.
- The code is a deliberate rebuild boundary: stable and large, while the code around it changes
  every day.
- The code is published separately, or it is a procedural macro (the `rust-macros` skill), which
  always needs its own `proc-macro` crate.

A long file, an area that "feels like a component", or one type that wants a home is not a
reason. Split a crate that grew too large into modules first.

## Add a crate

Read [references/new-crate.md](references/new-crate.md) for the seven steps when you add a
member. The traps:

- `cargo new --lib crates/<name>` writes `<key>.workspace = true` only for the keys that
  `[workspace.package]` defines, and `[lints] workspace = true` only when `[workspace.lints]`
  exists. Add a missing line by hand.
- Cargo never writes the `[workspace.dependencies]` entry. Add
  `<name> = { path = "crates/<name>" }`, with a `version` only when the policy needs one.
- Every `--locked` command fails until `cargo metadata --format-version 1 > /dev/null` records
  the member in `Cargo.lock`.
- Put `#![forbid(unsafe_code)]` in `src/lib.rs` unless the crate has hand-written `unsafe`,
  including an `#[unsafe(no_mangle)]` export.

Read [references/naming.md](references/naming.md) when you name or rename a crate.

## Restructuring an existing workspace

Read [references/dependency-direction.md](references/dependency-direction.md) when you audit an
inherited workspace, break an upward edge, or split, merge, or delete a crate.

| Situation | Move |
|-----------|------|
| Two crates need the same type | Move the type down to the lowest crate both already depend on |
| A lower crate calls into a higher crate | Invert with a trait defined in the lower crate |
| One crate does two unrelated jobs | Split it, and keep a re-export shim for one release |
| Two crates always change together and share no other consumer | Merge them |

Put a crate move in its own commit, apart from any behavior change: a move plus a behavior change
in one diff is unreviewable.

---
description: Use when changing how Cargo builds, resolves, or tests a Rust workspace - Cargo.lock and --locked policy, dependency updates and workspace inheritance, feature-unification surprises, resolver 3 and MSRV, rust-toolchain.toml, Cargo profiles, cargo nextest config, GitHub Actions pinning and caching, cdylib or staticlib builds with cargo rustc, .cargo/config.toml for cross targets, driving cargo from a host build system, or an edition migration with cargo fix --edition. Not for Android or iOS packaging; use rust-android-build or rust-ios-build.
license: BSD-3-Clause
metadata:
    github-path: skills/cargo-workflows
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 2c44a7706437a0cc3b54154d1f19e74a2b5536d2
name: cargo-workflows
---
# Cargo Workflows

Run Cargo from the workspace root. It reads `.cargo/config.toml` from the current
directory upward; a member directory can select different settings.

## `Cargo.lock` and `--locked`

Commit `Cargo.lock` for every package, libraries included. Pass `--locked` on every cargo command that CI, a host build
system, or an agent runs. Without it, a build silently resolves new versions and stops being
reproducible. Omit it only in a command whose purpose is to change the lock: `cargo update`,
`cargo add`, the `cargo metadata` resolve in the triage table below, and the scheduled
latest-dependencies job.

State why each lockfile change is needed:

| Task | Command |
|------|---------|
| Move one dependency | `cargo update -p <dep>` or `cargo update -p <dep> --precise <version>` |
| Preview a full update | `cargo update --dry-run` |
| Full update within the semver ranges | `cargo update`, as a dedicated change |

`cargo generate-lockfile` rebuilds an existing lockfile with the latest version of every package.
It is a full update, not a repair. Do not run it to fix a `--locked` failure.

| Symptom | Cause | Fix |
|---------|-------|-----|
| `cannot create the lock file ... because --locked was passed to prevent this` | `Cargo.lock` is not in the checkout | Commit it. `git ls-files --error-unmatch Cargo.lock` confirms that git tracks it. |
| `cannot update the lock file ... because --locked was passed to prevent this` | A manifest edit needs a lock change | Resolve without building: `cargo metadata --format-version 1 > /dev/null`. Review the lock diff, and vet each new package name (the `rust-security` skill, when it is installed). Commit the lock with the manifest, then rerun the original command with `--locked`. |
| A lock diff touches many unrelated packages | `cargo update` or `cargo generate-lockfile` ran | Revert the lock change and run `cargo update -p <dep>`. |

Never repair the lock by running any command that compiles (`cargo build`, `check`, `test`,
`clippy`, `doc`, `run`, `nextest`) without `--locked`. That command compiles the new packages and
runs their build scripts and proc macros before anyone vets them.

Library users resolve from `Cargo.toml`, not the library's lockfile. Add a scheduled
`cargo +stable update` and test job that reports without blocking merges.
It runs unreviewed build scripts.

Read [references/workspace-patterns.md](references/workspace-patterns.md) when you write this job
(runner, secret, and cache rules), lay out a workspace, add or pin a dependency, add a
member crate, review a `Cargo.lock` diff, or need a selective build or `cargo tree` command.

Read [the upgrade guide](references/rust-1.100-upgrade.md) when you upgrade
to Rust 1.100.

## Verification

Iterate with the narrowest check. Run the workspace gate once before commit or merge. Add the
feature, cross-target, and MSRV rows when the change touches features, `cfg`-gated code,
dependencies, or manifests.

| Claim | Check | A green result does not prove |
|-------|-------|-------------------------------|
| The crate type-checks | `cargo check --locked -p <crate>` | Codegen, linking, or monomorphization |
| The workspace builds and links | `cargo build --locked --workspace` | Other targets or feature sets |
| Lints pass | `cargo clippy --locked --workspace --all-targets -- -D warnings` | Unselected features and targets; rustdoc lints |
| Doc links resolve | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items` (the `rust-lints` skill owns this gate) | Doc-test behaviour |
| Tests pass | `cargo nextest run --locked --workspace` and `cargo test --locked --workspace --doc` | Filtered-out profiles and targets |
| Formatting | `cargo fmt --check` | Anything semantic |
| Each feature set builds | The matrix in the Feature flags section | Unlisted combinations |
| A shipping target compiles | `cargo check --locked --target <triple>` | Linking, or any test on that target |
| Target tests build | `cargo test --locked --no-run --target <triple>` | That any test passes on the target: report the lane as compile-only |
| The MSRV holds | `cargo +<msrv> test --locked --workspace` | Newer dependency versions |
| Newest dependencies work | The scheduled latest-dependencies job | The committed lock |

Pass `--all-targets` to clippy, or it skips tests, benches, and examples.

## CI

Read [references/ci-and-nextest.md](references/ci-and-nextest.md) when you write the cache steps
of a workflow or a `.config/nextest.toml`. It has both as complete examples, and the node24 and
cached-secret rules.

- Pin every action to a full commit SHA, with the version in a comment. A tag can move to new
  code.
- Use the node24 action majors; Node 20 runners are gone (the reference lists the versions).
- Never expose secrets to a job that writes a cache that pull requests can read: a tool can copy
  the environment into `target/`. The reference has the scoping and rotation rules.
- Do not set `RUSTFLAGS=-Dwarnings`. It rebuilds every dependency and replaces the per-target
  `rustflags` in `.cargo/config.toml`. Use `CARGO_BUILD_WARNINGS=deny` (Cargo 1.97+; an older
  Cargo ignores it and stays green) and keep the `rust-lints` gate
  `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.

## Toolchain, MSRV, and resolver

```toml
# rust-toolchain.toml. Example pin: use the release the project has tested.
[toolchain]
channel = "1.99.1"
components = ["rustfmt", "clippy"]
```

- Pin an exact release. Do not pin 1.98.0: it can emit a trait-object vtable with a null function
  pointer, and 1.98.1 fixes it. Do not pin 1.99.0: it miscompiles FFI `bool` return values on
  x86_64 (rust#163911), and 1.99.1 fixes it.
- In CI, run `rustup toolchain install` with no arguments (rustup 1.28+) before the first cargo
  command, so the install is a logged step and not an implicit side effect.
- Set `rust-version` in `[workspace.package]` and inherit it with `rust-version.workspace = true`.
  Clippy reads it and does not suggest newer APIs. Do not set `msrv` in `clippy.toml`: when the
  two differ, Clippy warns and uses `clippy.toml`.
- `rust-version` is a declaration, not proof. Resolver 3 prefers dependency versions that support
  it, but picks an incompatible version when no compatible version satisfies the requirement.
  Prove the MSRV with the real toolchain: `cargo +<msrv> test --locked --workspace`. The
  reference has the variants for a dev-dependency that needs a newer Rust.
- Use Cargo 1.96.1 or later for a third-party registry or SSH git source (the `rust-security`
  skill owns the rule); the reference has the offline vendor procedure for an older MSRV.

Resolver rules:

- Edition 2024 implies resolver `"3"` (MSRV-aware, Rust 1.84+) in a package manifest.
- A virtual workspace has no edition. Set `resolver = "3"` in its `[workspace]` table. Without it,
  Cargo falls back to resolver `"1"` and prints a `virtual workspace defaulting to` warning.
- Use `"2"` only when the workspace must build with Cargo older than 1.84.

## Feature flags

- Features are additive. Once any crate in a build enables a feature, it is on for every consumer
  in that build. Never use a feature to remove behaviour.
- Use `dep:<name>` in a feature list. A bare optional dependency name creates an implicit public
  feature with the same name.
- Add `required-features` to a `[[bin]]` that needs an optional dependency. Otherwise
  `cargo build --workspace` tries to build it and fails.
- If a dependency's default features change the output bit-for-bit (a GPU or SIMD backend), pin
  the feature set and write down why.
- Do not model loom as a Cargo feature. Loom code builds under `--cfg loom`.
- Test the project-owned matrix: default features, `--no-default-features`, and each supported
  feature set. Use `--all-features` only when the features are additive and the combination is a
  supported product.

Read [references/feature-resolution.md](references/feature-resolution.md) when you write the
feature matrix, a crate compiles features it did not ask for, a build passes with `-p <crate>` but
fails with `--workspace`, or an inherited dependency rejects or ignores `default-features = false`.

## Testing with cargo-nextest

When cargo-nextest is installed, use it as the main runner: it runs each test in its own process,
which isolates crashes. Without it, use `cargo test --locked`. Run
`cargo test --locked --workspace --doc` as its own step: nextest skips doc-tests.

- Keep network-dependent or otherwise flaky tests behind an opt-in profile and a single-threaded
  test group.
- Set `retries` only on the CI profile, with `flaky-result = "fail"` (nextest 0.9.131+). A retry
  that passes hides a race.

## Native artifacts and the FFI crate

`cargo rustc --crate-type` overrides the crate type for one invocation (stable since 1.64). The
manifest can then keep a plain Rust library, so `cargo build --workspace` does no linking work:

```bash
# cdylib for Android, JVM, desktop hosts; staticlib for iOS
cargo rustc --locked --profile <profile> --target <triple> \
    --crate-type cdylib -p <ffi-crate> --lib
```

- Find the produced file from the target directory's final-artifact path or from
  `--message-format=json`. Never read `deps/` or `build/` inside the target directory:
  `build.build-dir` (Cargo 1.91+) moves them. Cargo 1.100 changes their layout; use
  `compiler-artifact.filenames` and `executable` from JSON messages in scripts.
- Put `[profile.*]` sections only in the root manifest. Cargo ignores a member's profiles and
  warns `profiles for the non root package will be ignored`.
- The `rust-panic-safety` skill, when it is installed, owns the `panic` setting of a profile
  that builds an FFI artifact, and the guard pattern.
- Keep one FFI crate, a thin translation layer over the pure-logic crates.

Read [references/native-artifacts.md](references/native-artifacts.md) when you choose a profile
or the manifest crate type for a shared or static library, write FFI exports or a UniFFI crate,
or map a Cargo output name to the name a platform loader expects.

## Cross targets and host build systems

Read [references/cross-compilation.md](references/cross-compilation.md) when you set up
`.cargo/config.toml` for a non-host target or drive cargo from Gradle, Xcode, or CMake. The
`rust-cross-compilation` skill, when it is installed, owns target linkers, `CC_<triple>`, musl,
`cross`, and test runners.

## Rust edition

Edition 2024 is the latest edition as of Rust 1.100.0. Keep one edition in `[workspace.package]`,
inherited with `edition.workspace = true`, and bump it in a dedicated change. Two edition-2024
changes alter drop order with a clean build and green tests, and their lints go quiet after the
bump: run `-W rust_2024_compatibility` before `cargo fix --edition`.

Read [references/edition-migration.md](references/edition-migration.md) when you migrate a
crate, before the first command. It has the command sequence, the feature and target runs, the
migration order, the edition-2024 breaking changes, and the `rustfmt.toml` edition rule.

The `rust-security` skill, when it is installed, owns the `deny.toml` policy, `cargo audit`, and
advisory triage.


# Gate and configuration details

- [What each gate check proves](#what-each-gate-check-proves)
- [Cargo lints on Rust 1.100+](#cargo-lints-on-rust-1100)
- [`CARGO_BUILD_WARNINGS`](#cargo_build_warnings)
- [`clippy.toml` behavior](#clippytoml-behavior)
- [`rustfmt.toml` and the edition](#rustfmttoml-and-the-edition)
- [Export attributes and `forbid(unsafe_code)`](#export-attributes-and-forbidunsafe_code)

## What each gate check proves

- `--all-features` is correct only when the features are additive. With mutually exclusive
  features, run clippy once per supported feature set, or run
  `cargo hack clippy --each-feature --workspace --all-targets --locked -- -D warnings` when
  cargo-hack is installed. Give `cargo doc` and the tests the same feature sets as clippy. The
  `cargo-workflows` skill owns the feature matrix: default features, `--no-default-features`, and
  each supported set.
- Clippy does not run rustdoc lints. `[workspace.lints.rustdoc]` and
  `#![deny(rustdoc::broken_intra_doc_links)]` act only under `cargo doc`. On 1.98.1 a broken
  intra-doc link passes clippy and `cargo test --doc`, and fails only `cargo doc`. Without
  `--document-private-items`, rustdoc does not resolve the links in the docs of private items
  (verified on 1.98.1). Without `-D warnings`, `cargo doc` also passes warn-level rustdoc lints
  such as `private_intra_doc_links`.
- A host run does not lint code behind `#[cfg(target_os = "android")]` or another target cfg.
  Run `cargo clippy --locked --workspace --target <triple> --all-targets -- -D warnings` for
  each shipped target after `rustup target add <triple>`.
- `-- -D warnings` does not deny `linker_messages` (warn by default since 1.97, outside the
  `warnings` group). Clippy does not link, so only `cargo build` or `cargo test` prints linker
  output. The default policy keeps `linker_messages` at `warn`, because linker output changes
  with the host linker. To fail CI on linker output, run the build or test step with
  `CARGO_BUILD_WARNINGS=deny` (1.97+).
- A green gate on one toolchain can go red on the next: a clippy release adds lints to
  `pedantic` and `nursery`. Pin the toolchain, and re-run the gate after each bump.
- `cargo fmt --all` rewrites every file. When `cargo fmt --all -- --check` is red before your
  change, run `rustfmt --edition 2024 --config skip_children=true <file>` on the changed files
  only: plain `rustfmt <file>` also formats each module the file declares (1.99.1). Run the
  tests again after formatting.

## `CARGO_BUILD_WARNINGS`

`CARGO_BUILD_WARNINGS=deny` (or `build.warnings = "deny"` in `.cargo/config.toml`) fails a
local crate that has lint warnings. It does not change `RUSTFLAGS`, so the build cache
survives; add `--keep-going` to see the warnings of every crate. It also fails on
`linker_messages` and on `cargo doc` warnings (both verified on 1.98.1). Cargo older than 1.97
ignores it, and the gate passes silently. Use it only when the pinned toolchain is 1.97 or
newer; keep `-- -D warnings` otherwise. Set `CARGO_BUILD_WARNINGS=deny` in the CI job
environment, not in a checked-in `.cargo/config.toml`: the config key also turns every local
warning into an error while a developer iterates. Do not use `RUSTFLAGS=-Dwarnings`: it rebuilds
every dependency and replaces the rustflags from config files. `RUSTDOCFLAGS` also replaces
`build.rustdocflags` from config (verified on 1.98.1); when the config sets `rustdocflags`, use
`CARGO_BUILD_WARNINGS=deny` for the `cargo doc` line instead.

## `clippy.toml` behavior

Reasons behind the keys in the SKILL.md block:

- No `msrv` key. Clippy reads `rust-version` from each member's `Cargo.toml`; members inherit it
  with `rust-version.workspace = true`. Without it, clippy suggests APIs newer than the MSRV.
  Delete an existing `msrv` key: when it differs from `rust-version`, clippy warns and uses the
  `clippy.toml` value (verified on 1.99.1).
- `allowed-duplicate-crates` keeps `clippy::multiple_crate_versions` (the `cargo` group) from
  failing the build on a known duplicate major version.
- `avoid-breaking-exported-api = true` (the default) holds back every lint whose fix changes an
  exported signature. Set `false` only when no external crate consumes the API, such as an
  internal workspace crate or a binary.
- The `allow-*-in-tests` keys let test code unwrap; the `rust-discipline` skill owns the
  non-test rule.
- `doc-valid-idents` lists words that `clippy::doc_markdown` must not ask you to backtick.
  Clippy's default list already has SQLite, WebAssembly, GitHub, iOS, and macOS.
- The `std::ptr::read` entry does not catch the `<*const T>::read` method;
  `cast_ptr_alignment` catches the misaligned cast. The `for_each` entry is a catalog default,
  not a Rust rule: a `for` loop shows the side effect.

Run `cargo clippy --locked --workspace --all-targets` after you add a `disallowed-methods` or
`disallowed-types` entry. Clippy matches the resolved function, not a same-named method on
another type. It can ignore a path that it cannot resolve without a warning (`<*const T>::read`
on 1.98.1). Check that the entry fires where you expect.

Clippy reads only the nearest `clippy.toml`: a member's own file replaces the root file, and
every root key, `disallowed-methods` included, stops applying to that member (verified on
1.98.1).

With `allow-unwrap-in-tests = true`, an `#[expect(clippy::unwrap_used)]` on test code is
unfulfilled, and `-D warnings` fails on it (verified on 1.99.1). Do not add a per-test
suppression that the key already covers. The keys cover only `#[test]` functions and
`#[cfg(test)]` items: not a shared helper module under `tests/`, and not an `examples/` file.
Those take the inner `#![expect(...)]` of SKILL.md, Suppressions.

## `rustfmt.toml` and the edition

`cargo fmt` takes each crate's edition from `Cargo.toml` and ignores `edition` in
`rustfmt.toml`. A direct `rustfmt` call (an editor, a pre-commit hook) reads it. Without
`edition` or `style_edition`, a direct `rustfmt` call uses style edition 2015. It then sorts
`use a::{u8x, U8}` differently from `cargo fmt` on an edition-2024 crate, and CI fails on files
that look formatted. During a staged edition migration, keep both keys at the old edition until
the last crate moves; the `cargo-workflows` skill owns that sequence. Verified on 1.98.1: stable
rustfmt accepts `style_edition` without the unstable-option warning.

## Export attributes and `forbid(unsafe_code)`

`#[unsafe(no_mangle)]`, `#[unsafe(export_name)]`, and `#[unsafe(link_section)]` fail
`forbid(unsafe_code)` (1.98.1: ``usage of the unsafe `#[no_mangle]` attribute``; 1.99.1 drops
the `#[ ]`: ``usage of the unsafe `no_mangle` attribute``). An export that a proc macro
generates, such as jni 0.22's `#[jni_mangle]`, passes the lint (verified on 1.98.1); see
[lint-catalog.md](lint-catalog.md#lints-on-proc-macro-output) for why.

## Cargo lints on Rust 1.100+

Use `[workspace.lints.cargo]` for Cargo's own diagnostics; members still need
`[lints] workspace = true`. Use `[lints.cargo]` in a standalone package.
`unused_dependencies` warns by default. Run
`cargo check --locked --workspace --all-targets` on each supported feature set and
platform before removal. Cargo checks only dependencies whose possible consuming
build targets were selected; it does not check unused dev-dependencies.
`-- -D warnings` controls rustc or Clippy, not this manifest policy. Set
`unused_dependencies = "deny"` in the Cargo lint table when it is a required gate.
The `cargo-workflows` skill, when installed, owns `--message-format=json` and the upgrade workflow.

Test a required Cargo lint with a small package before enforcing it across a
workspace. For `unused_dependencies`, use two binaries, one feature-gated use,
one dependency under the active target cfg, and an unused dev-dependency:

| Command or change | Expected boundary on Cargo 1.100 |
| --- | --- |
| `cargo check --locked --bin one` | An unselected binary can use a shared dependency, so this cannot prove it unused |
| `cargo check --locked --all-targets` | The dependency used only behind a disabled feature can trigger the lint |
| Add `--features <feature>` | The dependency is used in this configuration |
| Remove the last use of an active target dependency | The required lint fails; an inactive target dependency is not judged on this host |
| Leave a dev-dependency unused | The Cargo lint does not diagnose it |

Require a failing negative case and a passing corrected case. A quiet selected
binary or inactive target is not evidence that the policy checks the whole graph.

Rust 1.100 also adds `rustdoc::invalid_markdown_table`. Run `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`
when a documentation table changes. `#[expect(dead_code)]` inside a dead parent
no longer reports an unfulfilled expectation merely because the parent owns the diagnostic.

Escape a pipe inside a Markdown table cell as `\|`, including inside code spans.
The unescaped cell `` `f(|x| x)` `` can lose columns in the rendered documentation.
Use a rustdoc fixture with that bad row and a corrected `` `f(\|x\| x)` `` row:
`rustdoc -D rustdoc::invalid_markdown_table <file.rs>` must reject the first and
accept the second. Rust compilation and Clippy do not exercise this lint.

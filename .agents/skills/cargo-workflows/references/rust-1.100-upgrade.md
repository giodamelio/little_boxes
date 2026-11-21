# Upgrade to Rust 1.100

Read this file when the workspace moves from Rust 1.99 or older to 1.100.
Keep the package MSRV separate from the CI toolchain pin. Run the MSRV job before
using a newly stable API in a library that supports an older compiler.

## Check compatibility first

| Symptom after the bump | Cause | Action |
| --- | --- | --- |
| A bound on `!` fails in an edition 2021 crate | Never-type fallback is now `!` on every edition | Add the intended type, such as `f::<()>()?`; allowing the old lint does not restore `()` |
| Two impls conflict for `Infallible` and `!` | `Infallible` is now an alias for `!` | Keep one impl; check public trait coherence before changing the API |
| An `if let` closure changes `Send` or drop behavior | More precise captures | Check the actual captured fields and their drop order; add a caller test |
| A trait object fails in a previously accepted position | More complete dyn-compatibility checks | Fix the trait object or use static dispatch; do not suppress the check |
| A `Box` conversion needs a type | More impls accept arbitrary allocators | Add a local `Box<T>` or allocator type annotation |
| ARM target-feature selection is rejected | The requested feature changes the target ABI | Select the correct target triple; do not force an ABI with `-Ctarget-feature` |

Run `cargo check --locked --workspace --all-targets`, tests, rustdoc, and Clippy
on each supported feature set and target. A host check does not cover target-specific code.

Keep a small before/after probe when the update changes language behavior.
Record the edition, both exact toolchains, the expected diagnostic code, and the
runtime assertion. Test the explicit fix on both versions. These Rust 1.100
transitions need more than a successful current-toolchain build:

| Probe | Before 1.100 | On 1.100 |
| --- | --- | --- |
| An irrefutable `if let (_, value) = pair` closure consumes one field | The whole pair can move into the closure | A separate field can remain available; test the actual `Send` bound and destructor log |
| An unconstrained `f()?` needs `Default` | Editions before 2024 report `dependency_on_unit_never_type_fallback`; an explicit allow can retain unit fallback | All editions fail with E0277 when `!` lacks the bound; the old lint allow does not repair it |
| Separate impls for `fn() -> !` and `fn() -> Infallible` | Distinct types permit both impls | E0119 reports the overlap |

Use `f::<()>()?` only when unit is the intended type. A successful explicit-type
probe checks that fix; it does not establish the intended API. Keep migration
probes separate from a full old-toolchain build when the current code uses new APIs.

## Read Cargo's artifact messages

Cargo 1.100 changes the intermediate build directory layout. Hashes identify
parent directories instead of only file names. Scripts that glob `deps/`,
`build/*/output`, or `.fingerprint/` can stop finding files or select stale files.

Use `--message-format=json` on the build or `cargo test --no-run` command. Read
`filenames` and `executable` from `compiler-artifact` messages. Read build-script
metadata from `build-script-executed` messages. Keep the target and profile in
the command. Final artifacts still go to the target directory; internal paths
are not an interface. Validate a cache change with two different feature sets.

Use a fixture whose build script emits different source for each feature set.
Build both sets in dev and release profiles, then build the first set again.
Execute each `compiler-artifact.executable` and check its expected output. Read
the generated source from `build-script-executed.out_dir`. Check the cached run
too: a fresh build alone cannot detect selection of a stale artifact.

## Check dependencies with Cargo lints

Cargo 1.100 adds `[lints.cargo]`, including `unused_dependencies` at warn level.
The `rust-lints` skill, when installed, owns `[workspace.lints.cargo]` policy.
Run `cargo check --locked --workspace --all-targets` for each supported feature
set and platform. A selected binary alone cannot prove that a shared dependency
is unused. Cargo does not report unused dev-dependencies. Review each finding
before removal; a green check does not prove that every dependency is used.

## Tool installation uses the published lockfile

Since Cargo 1.100, `cargo install` uses the lockfile in a published crate by
default. `--locked` is a no-op there; keep it in commands that also support older
Cargo. Pin the tool version as well. This change does not remove the need for
`--locked` on workspace build, test, or check commands.

## Sources

- [Rust 1.100 release notes](https://doc.rust-lang.org/1.100.0/releases.html#version-11000-2026-11-12)
- [Cargo build cache](https://doc.rust-lang.org/1.100.0/cargo/reference/build-cache.html)
- [Cargo JSON messages](https://doc.rust-lang.org/1.100.0/cargo/reference/external-tools.html#json-messages)
- [Cargo lints](https://doc.rust-lang.org/1.100.0/cargo/reference/lints.html)

# Release Compatibility Review

Read this when you classify a public crate release or repair an MSRV build.
Compare the previous published source with the candidate.
Apply the repository's published compatibility policy before this default table.

Contents:

- Version classification
- API diff with cargo-semver-checks
- Public API checklist
- Feature compatibility
- MSRV and platform compatibility
- Repair an MSRV build failure

## Version classification

| Candidate change | Default classification for `1.0.0` and later |
|---|---|
| Compatible bug fix, performance fix, or documentation fix | Patch |
| New compatible public item or opt-in feature | Minor |
| Remove, rename, or move a public item | Major |
| Add a variant to an exhaustive public enum | Major |
| Add any field, public or private, to a struct whose fields are all public, or a field to an exhaustive public enum variant | Major |
| Add or remove a private field when the struct already has one | Minor |
| Add `#[non_exhaustive]` to an existing enum, variant, or struct with no private field | Major |
| Add a required trait item or change a trait item signature | Major |
| Make a public trait no longer dyn compatible (the SemVer guide says "object safe") | Major |
| Tighten a public generic bound | Major |
| Change a stable layout or ABI guarantee | Major |
| Require `std` after supporting `no_std` | Major |
| Remove a feature or move existing public API behind a feature | Major |
| Raise MSRV | Possibly breaking; follow the published policy (the Cargo guide suggests minor) |
| Remove a supported platform | Possibly breaking; follow the published policy |
| Add a defaulted trait item or an inherent method | Possibly breaking; inspect downstream overlap |

For `1.0.0` and later, use patch for compatible fixes, minor for compatible additions, and major
for incompatible changes. Cargo uses the left-most non-zero version component as its
compatibility boundary: for `0.y.z`, `y` is the breaking position, and each `0.0.z` release is
incompatible with every other `0.0` release. Do not hide a breaking change in a patch release
because the crate is below `1.0.0`.

## API diff with cargo-semver-checks

Install the tool with `cargo install --locked cargo-semver-checks` when it is missing. It is a
CLI, not a manifest dependency.

```bash
cargo semver-checks -p <package> --baseline-version <previous-version>
```

`--baseline-version` looks up only crates.io. For another registry, pass
`--baseline-rev <previous-release-tag>`. Each release of the tool reads the rustdoc JSON of at
least the stable and beta Rust of its release date. Update the tool when you update the stable
toolchain.

## Public API checklist

Check these failure-prone changes explicitly:

- field visibility, field order, and layout guarantees;
- exhaustive structs, enums, and downstream matches;
- trait dyn compatibility and new required items;
- generic defaults, bounds, inferred types, and return-position `impl Trait` captures;
- blanket, foreign, inherent, and auto-trait implementations;
- function arity, safety, ABI, and panic contract;
- macro syntax, expansion paths, and exported helper names;
- re-export paths and feature-gated items;
- types from dependencies in public signatures;
- behavior, errors, and panics that the documentation promises.

A public item is not the complete public API.
Review public trait implementations and types that appear through re-exports.
Review generated macro output from a downstream crate context.
Review whether new inherent methods can collide with downstream trait methods.
Review whether a new impl changes method or type inference.

## Feature compatibility

Run the smallest feature matrix that covers each supported state:

```bash
cargo test --locked -p <package> --all-targets
cargo test --locked -p <package> --all-targets --no-default-features
cargo test --locked -p <package> --all-targets --no-default-features --features <feature-set>
```

Add `--all-features` only when the features are additive. Otherwise test each documented set, or
run `cargo hack test --each-feature` when `cargo-hack` is installed.

Treat feature names as public API.
A compatible release can usually add an opt-in feature.
Do not remove a feature in a compatible release.
Do not remove a feature from `default` when users can depend on its behavior.
Do not move an existing public item behind a feature in a compatible release.
Do not let enabling a feature break code that works without it.

An optional dependency creates an implicit feature with the same name by default.
Use `dep:<name>` under a stable user-facing feature when the dependency name is an internal
detail.

## MSRV and platform compatibility

Apply the project's documented MSRV policy and announce an increase in the changelog.

Treat a new operating-system, linker, native-library, CPU-feature, or runtime requirement as a
compatibility change.
Do not infer cross-target support from a host build.

## Repair an MSRV build failure

Find the cause before you raise the MSRV:

| Cause | Fix |
|---|---|
| A locked dependency needs a newer Rust | Re-resolve only that dependency: `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo update -p <dep>` (`fallback` is the default under resolver `"3"`), or `--precise <version>`. Do not run a full `cargo update` here: it moves every dependency just before publish. Review the lock diff. Vet each new package name with the `rust-security` skill, when it is installed, before you build or test. Then commit the lockfile and test again |
| Only dev-dependencies need a newer Rust | Prove the library with `cargo +<msrv> check --locked -p <package> --lib` and document that tests need a newer toolchain |
| The crate's own code needs a newer Rust | Restore compatibility, or raise the MSRV as a documented compatibility decision |
| The MSRV Cargo cannot parse `Cargo.toml` or `Cargo.lock` (TOML 1.1 syntax needs Cargo 1.94; lockfile v4 needs 1.78) | Use manifest syntax and a lockfile version that the MSRV Cargo reads. Or document that development needs a newer Cargo; Cargo rewrites the published manifest, so dependents keep the MSRV |

In a workspace with several MSRVs, use the per-package `cargo hack check --rust-version`
command of the `cargo-workflows` skill, when it is installed, when `cargo-hack` is installed.

## Official references

- [Cargo SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)
- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
- [Cargo Rust Version](https://doc.rust-lang.org/cargo/reference/rust-version.html)
- [Cargo Features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Cargo resolver, Rust version](https://doc.rust-lang.org/cargo/reference/resolver.html#rust-version)

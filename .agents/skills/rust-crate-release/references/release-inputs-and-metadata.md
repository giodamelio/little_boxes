# Release inputs and manifest metadata

Contents:

- Release inputs
- Read-only checks
- `[package]` metadata
- docs.rs metadata
- Official references

## Release inputs

Collect these values first:

| Input | Required evidence |
|---|---|
| Package | Exact Cargo package name and manifest path |
| Registry | `crates-io` or one configured registry name |
| Base | Previous published version and its source tag, or `none` for a first release |
| Candidate | Exact commit to release |
| Version | Proposed new version |
| MSRV | Effective `package.rust-version` and the project support policy |
| Features | Default set and every supported non-default combination |
| Targets | Supported target and operating-system policy |
| Mode | Registry package, binary distribution, or both |
| Channel | Local `cargo publish`, or a CI workflow that a tag or dispatch starts |
| Authority | Who can approve publish, tags, releases, uploads, signing, yanks, and owner changes |

## Read-only checks

Run these checks before any other step. They write nothing:

```bash
git status --short
git rev-parse HEAD
git tag --list --sort=-version:refname
cargo metadata --locked --no-deps --format-version 1
```

## `[package]` metadata

Verify these `[package]` values for the package that ships:

```toml
[package]
name = "<package>"
version = "<version>"
rust-version = "<msrv>"
description = "<one-line description>"
license = "<SPDX expression>"
repository = "<source URL>"
readme = "README.md"
```

Use `license-file` instead of `license` only for a nonstandard license, and confirm the file
enters the archive. Check every README link as it renders on the registry page. Do not set
`homepage` to the `repository` or documentation URL.

## docs.rs metadata

docs.rs announced that from 2026-05-01 it builds only the default target (`default-target`, else
`x86_64-unknown-linux-gnu`) unless `targets` lists more. Set `targets` explicitly when the public
API differs by target, or users see no documentation for the other targets:

```toml
[package.metadata.docs.rs]
features = ["<documented-feature>"]
targets = ["x86_64-unknown-linux-gnu", "<other-supported-target>"]
```

Use `all-features = true` only when the features are additive.

## Official references

- [Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [`cargo info`](https://doc.rust-lang.org/cargo/commands/cargo-info.html)
- [docs.rs build metadata](https://docs.rs/about/metadata)
- [docs.rs default target change](https://blog.rust-lang.org/2026/04/04/docsrs-only-default-targets/)

---
description: Use when preparing to publish a Rust crate or a Rust binary release, or recovering from a bad one - deciding a SemVer bump or MSRV change, checking cargo package and cargo publish output, ordering a workspace publish, setting up crates.io trusted publishing, deciding whether to yank a crate version, building a release archive with a release checksum, release SBOM, and attestation, or using cosign to sign release artifacts.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-crate-release
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 0bd9ac242008f207a9cd4336ee580449f8f59c90
name: rust-crate-release
---
# Rust Crate Release

A release has two independent modes: the registry mode publishes a source `.crate`, and the
binary mode distributes built assets. An authorization for one mode does not authorize the other.

## Safety rules

Published versions and uploaded assets are permanent, so these rules have no exception:

- Get explicit authorization before each external write. Name the exact package, version,
  registry, tag, asset, or account. External writes are: `cargo publish`, a commit or tag push,
  creating, editing, or finalizing a hosted release, an asset upload, signing or attestation,
  `cargo yank` and `--undo`, `cargo owner` changes, crate deletion, and crate settings such as a
  trusted publisher.
- Never add `--allow-dirty` or `--no-verify` to make a release pass. Both publish bytes that no
  check saw.
- Never put a registry token on the command line or in a log: history and CI logs keep it. If a
  token leaks, revoke it at once and create a new scoped token (crates.io:
  https://crates.io/settings/tokens).
- Never reuse a version after a partial or timed-out publish. The upload can have completed.
- Never release a different commit under an existing version, and never replace a named asset
  with different bytes. Users and caches already trust the first bytes.

Local checks, `cargo package`, and `cargo publish --dry-run` write nothing external. Run them
before you ask for authorization.

## Failure triage

| Symptom | Likely cause | Fix |
|---|---|---|
| `cargo info` output says `(from ./...)` | No `--registry`, so Cargo describes the local member | Pass `--registry <registry>` and read the result again |
| A file is missing from the archive, or archive tests fail with E0432 or E0433 | `include`/`exclude`, an ignore rule, a path-only dev-dependency, or an undeclared build input | Follow the triage table in the package-inspection reference |
| Dry run selects the wrong package | Workspace defaults or the current directory select another member | Pass `-p <package>` and `--registry` |
| docs.rs build fails or lacks a platform API | Feature, target, native dependency, or sandbox difference | Fix `[package.metadata.docs.rs]` or the build behavior, then release a new version if required |
| A binary digest, upload, or signature check fails | See the binary reference | Stop; follow its triage table |

## Completion report

Report: the published or prepared status of each mode; package, version, registry, commit, and
tag; the SemVer and MSRV decision with evidence; the exact checks and observed results; package
contents and archive size; for binaries, the target matrix, asset names, checksums, SBOM,
provenance, signature status, and the consumer verification of every downloaded asset; the
registry and docs.rs result; every external action; remaining risks and blocked checks.

Do not report a release as complete when only a dry run passed, or as failed only because the
client timed out.

## Release inputs

Read [references/release-inputs-and-metadata.md](references/release-inputs-and-metadata.md)
first: it lists the inputs, the evidence each needs, and the read-only checks, which end with
`cargo metadata --locked --no-deps --format-version 1`.

Stop if the candidate worktree is dirty. Use a clean worktree at the candidate commit when other
work must stay in place. `--locked` needs a committed `Cargo.lock`, libraries included.

Read the repository release policy, CI config, README, changelog, and recent tags. Do not invent
a tag prefix or changelog format.

## 1. Compare the published base with the candidate

Use the previous release tag as the base:

```bash
git diff --stat <previous-release-tag>..HEAD
git log --oneline <previous-release-tag>..HEAD
```

For a first release, review the complete public surface as the initial contract.

Inspect every change to the user contract, not only lines with `pub`: the checklist is in [references/compatibility.md](references/compatibility.md). Mark a public enum
that downstream code matches `#[non_exhaustive]` in its first release.

## 2. Classify the version change

Apply the repository policy first, then the Cargo SemVer guide and the classification table in
[references/compatibility.md](references/compatibility.md).

For `0.y.z`, `y` is the breaking position. Choose the larger bump for a possibly breaking change
unless policy and downstream evidence support a smaller one. Record the classification and its
evidence in the release notes.

Run `cargo semver-checks -p <package> --baseline-version <previous-version>`; the reference has
the install, the other-registry form, and the limits. Treat a reported break as real. A clean
result does not prove compatibility: it does not check behavior, documented panics, or every
macro change. If the tool cannot run, report the API diff as not run.

## 3. Verify MSRV, features, and targets

Keep the documented MSRV equal to the `rust-version` that CI verifies.

```bash
rustup toolchain install <msrv> --profile minimal
cargo +<msrv> test --locked -p <package> --all-targets
cargo +<msrv> test --locked -p <package> --doc
```

Run each `--doc` line only when the package has a library target. Below MSRV 1.96.1, vendor a
third-party registry or SSH git source with a newer Cargo and add `--offline` to each MSRV
command (the `rust-security` skill owns the rule).

When the MSRV build fails, find the cause before you raise the MSRV. Never run a full
`cargo update` as the fix: it moves every dependency just before publish. Read the cause and fix
table in [references/compatibility.md](references/compatibility.md) when the MSRV build fails or
a workspace has several MSRVs.

Test the smallest feature matrix that covers each supported state (the commands are in the
reference), and every supported target in CI. A host-only build does not prove target support.

## 4. Check manifest and documentation metadata

Read the `[package]` and docs.rs sections of the inputs reference when you write or review these
tables. Since 2026-05-01, docs.rs builds only the default target unless
`[package.metadata.docs.rs]` lists `targets`; list them when the public API differs by target.

## 5. Prepare the version and changelog

Update only the authoritative version field and respect workspace inheritance. Write the
changelog from the release diff, with breaking, MSRV, feature, and platform changes and migration
steps.

The archive must come from a clean release commit that holds only the version, lockfile,
changelog, and other release files. Land that commit on the remote default branch, with its own
authorization, before any publish: `.cargo_vcs_info.json` records its SHA in the registry forever.

```bash
git diff --cached --check
git diff --cached
```

Confirm that the version is free with `cargo info <package>@<version> --registry <registry>`
(always pass `--registry`, also for crates.io). Only `error: could not find` proves it is free; a
timeout or an authentication error does not.

## 6. Run repository gates

Run the repository's canonical format, lint, test, and build commands. Without them, run:

```bash
cargo fmt --all --check
cargo clippy --locked -p <package> --all-targets --all-features -- -D warnings
cargo test --locked -p <package> --all-targets --all-features
cargo test --locked -p <package> --doc --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked -p <package> --all-features --no-deps
```

Replace `--all-features` with the documented valid sets when features conflict. Keep the `--doc` line
for a library target: `--all-targets` does not run doctests.
Run the advisory gate (`cargo deny --config deny.toml --locked check advisories`, or else
`cargo audit`), or report it as not run. A `.crate` carries `Cargo.lock`, and
`cargo install --locked` builds with it, so a vulnerable locked dependency reaches binary users.
Run the MSRV and target checks after the final version change. A green CI run for a different
commit is not release evidence.

## 7. Inspect the exact package

List the files, review the list for missing inputs and for secrets and private keys, then create
and verify the archive:

```bash
cargo package --locked -p <package> --registry <registry> --list
cargo package --locked -p <package> --registry <registry>
cargo publish --locked -p <package> --registry <registry> --dry-run
```

Both must pass as written (see Safety rules). `cargo package` verifies only that the library and
binaries build. Read [references/package-inspection.md](references/package-inspection.md) when
you review the file list, inspect the `.crate`, or test the packaged source.

## 8. Distribute binary artifacts

Read [references/binary-distribution.md](references/binary-distribution.md) when users download
prebuilt executables, libraries, installers, or firmware. Report each mode separately.

## 9. Handle workspace release order

Read [references/workspace-publish.md](references/workspace-publish.md) when the release
publishes more than one workspace package: multi-package `cargo publish` is not atomic.

## 10. Request authorization and publish

Choose one channel per version: a direct local `cargo publish`, tagged only after the registry
shows the version, or a CI workflow that a tag push or dispatch starts, with no local publish.

For crates.io from GitHub Actions or GitLab.com, prefer Trusted Publishing to a stored token. Read
[references/trusted-publishing.md](references/trusted-publishing.md) when you write or review a
publish workflow or change the crate's publisher settings.

With the authorization request, present the completion-report facts so far, the changelog
summary, and the exact command: `cargo publish --locked -p <package> --registry <registry>`, or
the exact tag push that starts the workflow.

After approval, run `git status --short` and `git rev-parse HEAD` again. Stop if either differs
from the approved candidate, or if `git fetch origin && git merge-base --is-ancestor HEAD
origin/<default-branch>` fails. Run the approved command once. If it times out, check the
registry until the result is known, then stop the release chain; an immediate negative
`cargo info` does not prove a failure.

## 11. Verify the registry and tag the source

Run the same `cargo info` command. Confirm that the version line names no local `(from ./...)` path, and that docs.rs built: a
docs.rs failure is a release defect. For a direct release, tag the exact published commit. For a
CI release, confirm that the pushed tag points to the approved candidate, and create no second
tag. Use the repository's prefix and annotated-tag convention:

```bash
git tag -a <tag> -m "Release <package> <version>"
git show --no-patch --decorate <tag>
```

Get authorization before you push the tag. Never move an existing tag; if the remote tag exists
at another commit, stop. The release is complete only when the registry version and the remote
tag both match the approved source.

## 12. Recover from a bad release or change owners

If a secret entered the archive, revoke the secret first: a yank or a deletion does not remove it.
Read [references/recovery.md](references/recovery.md) when you consider a yank, a crate deletion,
an owner change, or a RustSec advisory for your own release; each needs its own authorization.

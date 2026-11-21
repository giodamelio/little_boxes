# Workspace Publish Order

Read this when one release publishes more than one package from a workspace. Each upload is an
external write; the safety rules in `SKILL.md` apply to every package in the set.

## Choose and order the packages

Publish only the packages that changed, plus the dependents that need their new versions.
Publish dependencies before dependents. For a dependency with `path` and `version`, confirm that
`version` selects the release the dependent needs.

## Dry-run the whole set

Verify the whole ordered set in one dry run before the first upload:

```bash
cargo publish --locked --dry-run --registry <registry> -p <dependency> -p <dependent>
```

Since Cargo 1.90, this resolves each new workspace version from a local overlay. A dry run of the
dependent alone fails until the dependency is in the index: `no matching package named` for a new
crate, and `failed to select a version for the requirement` for a new version of a published
crate.
Run the package listing and archive checks (`SKILL.md` section 7) for each package.

## Publish

Multi-package `cargo publish` is not atomic: a server error leaves earlier packages published.
Publish one package at a time when you need a stop point after each upload. Request authorization
with the complete ordered list. Stop after the first failed or uncertain publish.

## Official references

- [`cargo publish`](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
- [Cargo changelog](https://doc.rust-lang.org/nightly/cargo/CHANGELOG.html)

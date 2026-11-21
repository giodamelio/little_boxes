# Package inspection

Read this when you review the `cargo package --list` output, inspect the `.crate`, or test the
packaged source.

Contents:

- Archive location
- Review the file list
- Test the packaged source
- Triage
- Official references

## Archive location

Read `target_directory` from the `cargo metadata` output. `cargo package` writes the archive to
`<target_directory>/package/`, which is not `./target` when `CARGO_TARGET_DIR` or
`build.target-dir` is set. Inspect the `.crate` there; `cargo publish` does not keep one since
Cargo 1.93. crates.io rejects a compressed `.crate` over 10 MB.

## Review the file list

Review the complete `cargo package --locked -p <package> --registry <registry> --list` output.
Look for missing source, generated files, README, license, and build inputs. Look for secrets,
credentials, private keys, internal notes, large fixtures, and repository-only files. Use
`include` or `exclude` only when the default set is wrong; `include` overrides `exclude`.

## Test the packaged source

`cargo package` verifies only that the library and binaries build. Test the packaged source when
tests, examples, build scripts, or generated files can differ from the workspace:

```bash
release_tmp="$(mktemp -d)"
tar -xzf "<target_directory>/package/<package>-<version>.crate" -C "$release_tmp"
(
  cd "$release_tmp/<package>-<version>"
  cargo test --locked --all-targets
  RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
)
```

Cargo removes a path dev-dependency that has no `version` from the published manifest. Tests that
use it pass in the workspace and fail from the archive with E0432 (a `use` import) or E0433 (a
path such as `helper::f()`). Keep the directory until diagnosis ends.

## Triage

| Symptom | Likely cause | Fix |
|---|---|---|
| `cargo package --list` misses a file | `include`, `exclude`, or a VCS ignore rule removes it | Fix the manifest and list again |
| Archive tests fail with E0432 or E0433 on a test helper crate | A path-only dev-dependency is removed from the published manifest | Give it a published `version`, or keep those tests out of the archive and document it |
| Package verifies in the workspace but not from the archive | Hidden path, generated file, or undeclared build input | Include the input or generate it in `OUT_DIR` |

## Official references

- [`cargo package`](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
- [Cargo changelog](https://doc.rust-lang.org/nightly/cargo/CHANGELOG.html)

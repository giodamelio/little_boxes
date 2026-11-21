# Unresolved imports: E0432, E0433, and E0425

Deep material for [SKILL.md](../SKILL.md). Read the `note:` and `help:` lines before you add a
dependency. Stop at the first step that matches:

1. `note: found an item that was configured out` with "the item is gated behind the `f`
   feature". The feature is off. Read the file path under the note.
   - The item is in a dependency: enable its feature with `cargo add <crate> --features f`, or
     add `f` to that dependency's `features` list. Do not rewrite the import.
   - The item is in this crate: the caller lacks the item's gate. Put the same
     `#[cfg(feature = "f")]` on the caller, or build with `--features f`. Do not add `f` to
     `default` to silence the error; that hides the break in the build without the feature.
2. `help:` offers a `use` path ("consider importing") or says "a similar path exists". This
   covers `cannot find type` and `cannot find value` (E0425 or E0433) and E0432. Take that path:
   `std::...`, `crate::...`, or `super::...`. Check `std` and this crate before any new crate.
3. The first path segment is a module of this crate: `grep -rn 'mod <segment>' src` finds it.
   Write the path from `crate::` or `super::`.
4. The first path segment is a crate that `Cargo.toml` already lists. The item path is wrong, or
   the item exists only in another version. Read the source of the version in `Cargo.lock`, not
   the documentation of the latest release. Use the package name as `Cargo.toml` writes it: the
   path segment `tokio_util` is the package `tokio-util`.

   ```bash
   cargo metadata --format-version 1 --locked \
     | jq -r '.packages[] | select(.name == "<package>") | "\(.version) \(.manifest_path)"'
   ```

   If `jq` is not installed, `cargo tree -i <package> --locked --depth 0` prints the locked
   version.
5. The crate is not in `Cargo.toml`. Add a crate only when the code was written against that
   crate, or the task asks for a new dependency. Otherwise use `std` or an existing dependency.
   Run `cargo info <name>`: it prints the description, version, `rust-version`, repository, and
   features of that exact package, or "could not find". It proves that the name exists, not that
   it is the crate the code expects. Compare its `repository` and `description` with that crate.
   If they do not match, or the name came only from rustc's `help:` line, do not add it; ask the
   user. Otherwise run `cargo add <name>`.

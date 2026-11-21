# Creating a new crate

The full procedure behind SKILL.md, "Add a crate". Run it from the workspace root.

1. **Choose the layer.** Match the crate's contents to the layer table, and list
   the crates it must depend on. Its layer must be at or above the highest layer
   among them. If that puts it above a crate that will use it, the design is
   wrong: stop and re-read the direction rules in SKILL.md.

2. **Create the package** from the workspace root. `crates/` is the common
   layout; use the directory the workspace already uses for members.

   ```bash
   cargo new --lib crates/<crate-name>
   ```

   Use `cargo init --lib <dir>` for a directory that already exists. Either
   command adds the member to `[workspace] members` unless a glob already
   matches it. It keeps a sorted list sorted and appends to an unsorted one, so
   check the result.

3. **Read the generated manifest.** Cargo writes `<key>.workspace = true` only
   for the keys that `[workspace.package]` defines, and `[lints] workspace =
   true` only when `[workspace.lints]` exists. Add a missing inheritance line by
   hand. The `rust-lints` skill owns the lint set that the `[lints]` table
   inherits.

4. **Add the `[workspace.dependencies]` entry by hand.** Cargo never writes it.
   Without it, a dependent that writes `<crate-name> = { workspace = true }`
   fails with "error inheriting `<crate-name>` from workspace root manifest's
   `workspace.dependencies.<crate-name>`".

   ```toml
   [workspace.dependencies]
   <crate-name> = { path = "crates/<crate-name>" }
   ```

   Add a matching `version` field when you publish the crate, or when the
   workspace denies wildcard dependency requirements even with
   `publish = false`. Read the local policy before you choose the path-only
   form. A `version` that does not match the member version makes every cargo
   command fail with `failed to select a version for the requirement`.

5. **Add dependencies through `workspace = true`.** This is the default because
   it keeps each version requirement in one place.

   ```toml
   [dependencies]
   core-types = { workspace = true }

   [dev-dependencies]
   golden-test-support = { workspace = true }
   ```

6. **Add the safety attribute** to `src/lib.rs`:

   ```rust
   #![forbid(unsafe_code)]
   ```

   Omit it only when the crate has hand-written `unsafe`, including an
   `#[unsafe(no_mangle)]` export: a C ABI or JNI export crate, a driver, a crate
   over a raw C API. A crate that calls C only through a safe wrapper crate
   keeps it. The `rust-unsafe` skill covers the crates that omit it: each `unsafe` block there
   carries a `// SAFETY:` comment.

7. **Record the member in `Cargo.lock`, then verify.** A new member changes the
   lockfile, so every `--locked` command that resolves the graph (`check`,
   `clippy`, `tree`, `test`) fails with `cannot update the lock file ... because
   --locked was passed` until you update it once. Resolve it without a build with
   `cargo metadata --format-version 1 > /dev/null`; the `cargo-workflows` skill owns the
   repair. Expect the lock diff to hold
   the new package and its new dependencies only. Then verify:

   ```bash
   cargo clippy --locked -p <crate-name> --all-targets -- -D warnings
   cargo tree --locked -p <crate-name> -e normal,build --target all
   ```

   Every crate in the tree must sit at or below the new crate's layer.

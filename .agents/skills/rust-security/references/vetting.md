# Vetting steps for a new or changed dependency

Contents:

- Steps 1 to 6 for a new package name
- A version bump of a package already in the lockfile

The gate and its scope are in section 5 of `SKILL.md`. Until step 3 passes for every new
`Cargo.lock` package, do not compile the workspace and keep rust-analyzer off it.

## Steps 1 to 6 for a new package name

1. **Create the candidate lockfile.** After you edit `Cargo.toml`, resolve once without
   `--locked`. After `cargo update`, the lockfile is already written. Then read the diff:

   ```bash
   cargo metadata --format-version 1 > /dev/null
   git diff -- Cargo.lock
   ```

   Every new `[[package]]` name is a package to vet. Do not use `cargo update -p <crate>`
   for a crate that is not in the old lockfile. Cargo cannot select it yet. Do not run a
   `--locked` check before this step, because the old lockfile does not hold the new graph.
2. **Check the identity.** Compare the name character by character with the intended
   crate. Look for an added word or character (`faster_log` for `fast_log`), a `-rs` or
   `-rust` suffix, or a plural. crates.io treats `-` and `_` as the same name, so a swap
   alone cannot squat. Read the first-publish date, the download count, and the repository
   URL, and check the owners on the crate page:

   ```bash
   curl -s -A '<tool> (<contact>)' https://crates.io/api/v1/crates/<crate> \
     | jq -r '.crate | "\(.created_at) \(.downloads) \(.repository)"'
   ```

   A crate first published less than 7 days ago (a default threshold; set your own) under
   a name close to a popular crate is a red flag. Reject it, or read every file of it in
   step 3.
3. **Read the published source, not the repository.** The `.crate` can differ from the Git
   tree. Run `cargo fetch --locked`, then find the unpacked source of each new package:

   ```bash
   cargo metadata --locked --format-version 1 \
     | jq -r '.packages[] | select(.name == "<crate>" and .version == "<version>") | .manifest_path'
   ```

   Triage every new package. Grep the directory of that `manifest_path` for code that
   reaches outside the process, and read each match:

   ```bash
   grep -rnE 'Command::new|TcpStream|UdpSocket|reqwest|ureq|hyper|env::var|home_dir|include_bytes!|[A-Za-z0-9+/=]{100,}' <dir>
   ```

   List the packages that run code at compile time:

   ```bash
   cargo metadata --locked --format-version 1 | jq -r '.packages[]
     | select(any(.targets[].kind[]; . == "custom-build" or . == "proc-macro"))
     | "\(.name) \(.version)"'
   ```

   Read the build script of each new package in full, with every file it includes. Read
   every file of a package that is a proc macro, has a grep match that its purpose does not
   explain, was first published or released less than 7 days ago, or has few downloads or
   a new owner. Look for network access, `std::process::Command`, environment reads,
   home-directory and key-file paths, embedded blobs, and file writes outside `OUT_DIR`. A
   pure utility crate that opens a socket is a red flag.
4. **Write a caret requirement at the reviewed version** (`name = "1.2.3"`). The committed
   `Cargo.lock` holds the exact version. Do not vet with `=1.2.3`: it stops
   `cargo update -p` from taking a security fix, and in a published library every other
   requirement must accept exactly that version, which a yank breaks. Use `=` only for a
   crate and its companion proc-macro crate.
5. **Run the policy on the candidate graph.** Run
   `cargo deny --config deny.toml --locked check`. Reject the dependency if it fails.
6. **Justify the dependency.** Check whether the standard library or a crate already in
   the graph does the job. Every new crate widens the attack surface.

## A version bump of a package already in the lockfile

Check the release date:

```bash
curl -s -A '<tool> (<contact>)' https://crates.io/api/v1/crates/<crate>/<version> | jq -r .version.created_at
```

Read the changes of a release under 7 days old or one that adds a build script or a
proc-macro dependency: `diff -ru <old-dir> <new-dir>` on the two unpacked directories.

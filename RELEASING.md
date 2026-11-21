# Releasing little_boxes

This is the release runbook. Work through it top to bottom.

- To keep a record, copy it to `tmp/release-X.Y.Z.md` and tick the boxes there. `tmp/` is ignored by git, so the copy never ends up in a commit.
- Every step says what you should see. When you see something else, follow its **If not** line, and do not continue past a failed step.
- Run every command from the repository root, inside `nix develop`.

## 0. Check the prerequisites

Do these once per machine. They stay true between releases.

- [ ] **0.1** Enter the dev shell: `nix develop`.
  - Expect: `git-cliff --version`, `committed --version`, `prek --version` and `cargo set-version --help` all run.
  - If not: you are outside the dev shell, or `flake.nix` is broken. Stop.
- [ ] **0.2** Check that GitHub CLI is logged in: `gh auth status`.
  - Expect: `Logged in to github.com`, with the `repo` and `workflow` scopes.
  - If not: run `gh auth login`.
- [ ] **0.3** Check that you can publish to crates.io: `test -f ~/.cargo/credentials.toml && echo ok`.
  - Expect: `ok`.
  - If not: create a token on [the crates.io API tokens page](https://crates.io/settings/tokens) with the `publish-update` scope for `little_boxes`, then run `cargo login`.
- [ ] **0.4** Check that you have a nixpkgs fork and a local checkout of it: `gh repo view <your-user>/nixpkgs --json name`.
  - Expect: `{"name":"nixpkgs"}`, and a checkout whose `upstream` remote is `https://github.com/NixOS/nixpkgs`.
  - If not: run `gh repo fork NixOS/nixpkgs --clone=false`, then clone your fork and add the upstream remote.

## 1. Decide what to release

- [ ] **1.1** Fetch the remote: `jj git fetch`.
- [ ] **1.2** Check that your `master` matches the remote: `jj log -r 'master | master@origin'`.
  - Expect: one commit, carrying both `master` and `master@origin`.
  - If not: someone pushed to `master`, or you have an unpushed local `master`. Reconcile them before going on. Stop.
- [ ] **1.3** Find the previous release tag: `jj tag list 'glob:v*'`. Set `PREV` to the highest version shown:

  ```sh
  export PREV=v1.12.1
  ```

- [ ] **1.4** List the changes to release: `jj log -r 'master..@'`.
  - Expect: every change you mean to release, each described, and nothing else.
  - If not: finish, describe or abandon the stray changes. Then repeat 1.4.
- [ ] **1.5** Run every hook, including the commit message lint: `prek run -a`.
  - Expect: every hook `Passed`, including `committed`.
  - If `committed` fails: reword each reported commit with `jj desc -r <change>`, following `AGENTS.md`. Then repeat 1.5.
  - If another hook fails: fix it in a commit of its own below the release. Then repeat 1.5.
- [ ] **1.6** Preview the changelog: `git-cliff --unreleased`.
  - Expect: an entry for every change a user would notice, and no internal changes.
  - If an entry is missing: the commit has a type that git-cliff leaves out, such as `chore` instead of `fix`. Reword it, then repeat from 1.5.
  - If an entry should not be there: reword the commit to a type that is left out, or add a `Changelog: skip` trailer. Then repeat from 1.5.
  - If there are no entries at all: there is nothing to release. Stop.
- [ ] **1.7** Get the next version: `git-cliff --bumped-version`. Set `NEXT` to it, without the `v`:

  ```sh
  export NEXT=1.13.0
  ```

  - Expect: a `feat` bumps the minor version, `fix` or `perf` the patch version, and a `!` breaking change the major version.
  - If not: a commit type is wrong. Fix it and repeat from 1.5.

## 2. Create the release commit

- [ ] **2.1** Start the release commit on top of the last change to release: `jj new <top-change>`.
  - Expect: `jj st` shows an empty working copy whose parent is that change.
- [ ] **2.2** Bump the version: `cargo set-version "$NEXT"`.
  - Expect: `Upgrading little_boxes from <old> to <NEXT>`. `jj diff --stat` shows one changed line in each of `Cargo.toml` and `Cargo.lock`, and nothing else.
  - If `Cargo.lock` changed more than one line: a dependency resolved differently. Run `jj restore Cargo.lock` and investigate before going on. Stop.
- [ ] **2.3** Generate the changelog: `git-cliff --tag "v$NEXT" --output CHANGELOG.md`.
  - Expect: `jj diff CHANGELOG.md` adds one `## [NEXT] - <today>` section with the same entries as 1.6, and changes nothing else.
- [ ] **2.4** Describe the release commit: `jj desc -m "chore(release): $NEXT"`.
- [ ] **2.5** Start an empty change on top: `jj new`.
  - Expect: the release commit is now `@-`, and the colocated git `HEAD` points at it. Steps 3.5 and 7.1 rely on that.

## 3. Verify the release commit

Every check runs against `@-`, the release commit.

- [ ] **3.1** Check formatting: `cargo fmt --check`.
  - Expect: no output.
- [ ] **3.2** Run clippy: `cargo clippy --all-targets -- -D warnings`.
  - Expect: no warnings other than the `manpages/completions built at` note from `build.rs`.
- [ ] **3.3** Run the tests: `cargo test`.
  - Expect: `test result: ok` for every suite.
- [ ] **3.4** Build the Nix package and check its version: `nix build && ./result/bin/little_boxes --version`.
  - Expect: `little_boxes <NEXT>`.
- [ ] **3.5** Check the man page and completions: `man ./result/share/man/man1/little_boxes.1.gz`, then `ls ./result/share/{bash-completion,zsh,fish}`.
  - Expect: every flag from `little_boxes --help` appears in the man page, and the completion files exist. `build.rs` generates them on every build, so there is nothing to commit.
- [ ] **3.6** Do a dry run of the crates.io publish: `cargo publish --dry-run --registry crates-io`.
  - Expect: `Uploading little_boxes v<NEXT>`, then `warning: aborting upload due to dry run`.
  - `--registry crates-io` is required. Without it, cargo refuses because `.cargo/config.toml` replaces crates.io with the vendored sources.

**If any check in this section fails:** run `jj abandon <release-commit>`, fix the problem in a new commit below where the release commit was, and restart at 1.4. Never fix forward on top of the release commit.

## 4. Run CI on the release commit

`master` only moves once CI has passed on the exact commit it will point at.

- [ ] **4.1** Create a temporary bookmark: `jj bookmark create "release-$NEXT" -r @-`.
- [ ] **4.2** Push it with your `jj push` alias, which runs `prek run -a` first: `jj push --bookmark "release-$NEXT"`.
  - Expect: the hooks pass, and jj reports adding the bookmark on the remote.
  - If the hooks fail: go back to 1.5.
- [ ] **4.3** Wait for CI on that commit:

  ```sh
  gh run list --commit "$(jj log -r @- --no-graph -T commit_id)"
  gh run watch <run-id> --exit-status
  ```

  - Expect: the Rust, Nix and Commits workflows all succeed.
  - If not: read the failing job with `gh run view <run-id> --log-failed`, then follow the **If any check fails** rule from section 3.

## 5. Publish master and the tag

- [ ] **5.1** Move `master` to the release commit: `jj bookmark set master -r @-`.
  - Expect: `jj log -r master` shows `chore(release): <NEXT>`.
- [ ] **5.2** Push `master`: `jj push --bookmark master`.
  - Expect: jj reports moving `master` forward on the remote.
  - If the push is rejected: `master` moved on the remote since 1.1. Stop, and restart from 1.1.
- [ ] **5.3** Tag the release commit: `jj tag set "v$NEXT" -r master`.
- [ ] **5.4** Check that the tag and `master` are on the same commit: `jj log -r "master & v$NEXT"`.
  - Expect: exactly one commit. A tag made before the last rewrite of the release commit is left on an old version of it, which is what happened to `v1.12.1`.
- [ ] **5.5** Push the tag, which starts the release workflow: `jj git push --tag "v$NEXT"`.
- [ ] **5.6** Delete the temporary bookmark locally and on the remote: `jj bookmark delete "release-$NEXT"`, then `jj git push --deleted`.

## 6. Check the GitHub release

- [ ] **6.1** Watch the release workflow:

  ```sh
  gh run list --workflow release.yml --limit 1
  gh run watch <run-id> --exit-status
  ```

  - Expect: success.
  - If **Check the release notes list changes** failed: git-cliff found no entries for the tag. The tag is on the wrong commit, or 1.6 was skipped. Stop and investigate. Do not move a published tag without deciding that deliberately.
  - If a later step failed for a reason outside the code, such as a network or runner error: re-run it with `gh run rerun <run-id>`, or start it again with `gh workflow run release.yml -f tag="v$NEXT"`.
- [ ] **6.2** Check that the release notes match the changelog:

  ```sh
  diff <(gh release view "v$NEXT" --json body --jq .body) <(git-cliff --current --strip all)
  ```

  - Expect: no differences, apart from trailing whitespace.
- [ ] **6.3** Check the assets: `gh release view "v$NEXT" --json assets --jq '.assets[].name'`.
  - Expect exactly these five:
    - `little_boxes-<NEXT>-linux-amd64.tar.gz`
    - `little_boxes-<NEXT>-linux-arm64.tar.gz`
    - `little_boxes-<NEXT>-darwin-amd64.tar.gz`
    - `little_boxes-<NEXT>-darwin-arm64.tar.gz`
    - `checksums.txt`
- [ ] **6.4** Smoke-test the Linux build:

  ```sh
  dir=$(mktemp -d)
  gh release download "v$NEXT" --pattern '*linux-amd64.tar.gz' --dir "$dir"
  tar -xzf "$dir"/*.tar.gz -C "$dir"
  "$dir/little_boxes" --version
  ls "$dir/CHANGELOG.md"
  ```

  - Expect: `little_boxes <NEXT>`, and `CHANGELOG.md` is in the archive.

## 7. Publish to crates.io

- [ ] **7.1** Publish from the release commit: `cargo publish --registry crates-io`.
  - Expect: `Published little_boxes v<NEXT>`.
  - If cargo reports uncommitted changes: `@` is not empty. Run `jj new master` and repeat 7.1.
- [ ] **7.2** Check that crates.io has it: `cargo search little_boxes --limit 1 --registry crates-io`.
  - Expect: `little_boxes = "<NEXT>"`. The index can take a minute to update.

## 8. Update nixpkgs

The package is `pkgs/by-name/li/little_boxes/package.nix`. It builds from the GitHub tag with the committed `vendor/` directory, so only `version` and the source `hash` change.

- [ ] **8.1** Look for an update pull request from the nixpkgs update bot:

  ```sh
  gh pr list --repo NixOS/nixpkgs --state open --search "little_boxes in:title"
  ```

  - If one exists for `<NEXT>`: review it instead of opening your own. Check that it changes only `version` and `hash`, build it as in 8.4, and approve it. Then skip to section 9.
- [ ] **8.2** In your nixpkgs checkout, branch from the latest upstream `master`:

  ```sh
  git fetch upstream master
  git switch --create "little_boxes-$NEXT" upstream/master
  ```

- [ ] **8.3** Update the package: `nix run nixpkgs#nix-update -- little_boxes --version "$NEXT"`.
  - Expect: `git diff` changes only the `version` and `hash` lines of `package.nix`.
- [ ] **8.4** Build it, which also runs `versionCheckHook`: `nix-build -A little_boxes && ./result/bin/little_boxes --version`.
  - Expect: the build succeeds, and prints `little_boxes <NEXT>`.
- [ ] **8.5** Commit with the nixpkgs title format: `git commit --all --message "little_boxes: ${PREV#v} -> $NEXT"`.
- [ ] **8.6** Push to your fork, then open the pull request with the nixpkgs template:

  ```sh
  git push --set-upstream origin "little_boxes-$NEXT"
  gh pr create --repo NixOS/nixpkgs --base master --web
  ```

  - In the template, tick the checks you ran: built on your platform, and tested the binary. Link the GitHub release for `v<NEXT>`.

## 9. Cleanup


- [ ] **9.1** Return to a clean working copy on the new `master`: `jj new master`.

## Know what is not automated

These are separate projects, not release steps:

- **Homebrew:** there is no formula in homebrew-core, and `.goreleaser.yml` has no tap configuration.
- **Man page and completions in the GitHub archives:** the archives contain only the binary, `LICENSE`, `README.md` and `CHANGELOG.md`. Only the Nix package installs the man page and completions.

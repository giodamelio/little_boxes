# little_boxes

This project gets its Rust toolchain, devshell, git hooks and checks from [rust-toolchains](https://github.com/giodamelio/rust-toolchains) through `rust-toolchains.flakeModules.default`. Most tooling is defined there, not in this repo. `flake.nix` holds only what this project adds on top.

## Keep the shared toolchain

The user runs kache, a rustc wrapper cache that keys on the full `rustc -vV` output. Every project that uses rust-toolchains gets a byte-identical rustc, so they share cache entries.

- Do not add a `rust-toolchain.toml`, a rustup override, or another Rust toolchain to the devshell. Any of these breaks cache sharing.
- To switch to nightly, set `rust-toolchains.channel = "nightly";` in `flake.nix`.
- To pick up a newer pinned toolchain and tooling, run `nix flake update rust-toolchains`.
- Do not set `CARGO_TARGET_DIR` or any `KACHE_*` variable. The user's shell sets them per project.

## Work inside the devshell

Enter it with `nix develop`, or let direnv load it from `.envrc`. Run every cargo command inside it.

It provides:

- `cargo`, `rustc`, `clippy`, `rustfmt` and `rust-analyzer`, from the pinned toolchain
- `cargo-nextest` and `cargo-test-auto`
- `alejandra` and `prek`
- `cargo-unused-deps`, `cargo-shear` and `cargo-unused-dev-deps`
- `cargo-deny` and `cargo-deny-check`
- `skills-install` and `skills-bump`
- `git-cliff`, `committed`, `actionlint`, `cargo-edit` and `cargo-upgrade-vendored`, which this project adds
- `treefmt`, the formatter `nix fmt` runs, which formats Nix, Rust and YAML

To add a project-specific tool, append it to `rust-toolchains.devShell.packages` in `flake.nix`.

## Keep vendor/ in step with Cargo.lock

`vendor/` holds every crate the project depends on, and `.cargo/config.toml` points cargo at it instead of crates.io. The nixpkgs package builds from `vendor/` with no network access, and so does `packages.default`, so a stale `vendor/` breaks both.

- To change dependencies, run `cargo-upgrade-vendored`, which takes the same arguments as `cargo upgrade` and re-vendors afterwards.
- For a lockfile-only update, such as `cargo update --package <crate>`, move `.cargo/config.toml` aside, run the update, put the file back, and run `cargo vendor --locked vendor`.
- Commit `vendor/` in the same commit as the `Cargo.lock` change.
- Never delete `vendor/` or `.cargo/config.toml`.

### Make jj see every re-vendored file

Files from crates.io all carry the same fixed modification time. When `cargo vendor` replaces a file with a new version of the same size, such as a `lib.rs` whose only change is the version number, jj sees an unchanged size and time and does not snapshot it. The commit then keeps the old file. Everything built from the working tree still passes, including `nix build` and `nix flake check`, but CI checks out the commit, and cargo fails with `the listed checksum of ... has changed`.

After every `cargo vendor` or `cargo-upgrade-vendored`, before you describe the commit:

1. Touch every vendored file, so jj reads them all again: `find vendor -type f -exec touch {} +`.
2. Check that `jj diff --stat` now lists each file the update changed.
3. Build the commit itself, not the working tree, the way CI does: `nix flake check "git+file://$PWD?rev=$(jj log -r @- --no-graph -T commit_id)"`. Run it after `jj new`, so `@-` is the vendoring commit.

## Install the agent skills

`agent-skills.lock.json` lists the Rust agent skills this project uses and pins their source repo to one commit. The skill files live in `.agents/skills/`, and `.claude/skills/<name>` is a relative symlink to each one. Commit the lockfile, the skill files and the symlinks.

- Entering the devshell checks the installed skills against the lockfile, offline. If any are missing or stale, it says how many and tells you to run `skills-install`. Nothing is downloaded automatically.
- `skills-install` downloads each missing or stale skill at the pinned commit and recreates its `.claude/skills` symlink. It skips skills that are already current, so running it again does nothing.
- To add a skill, add `"name": null` to the lockfile's `skills` object and run `skills-bump` with the lockfile's current `ref`, which fills in its tree hash. Then run `skills-install`. To drop one, delete its entry, its `.agents/skills/<name>` directory and its `.claude/skills/<name>` symlink.
- Run `skills-bump` to move the pin to the source repo's latest release, or `skills-bump TAG` for a specific tag or commit. Then run `skills-install`.
- `skills-install` needs GitHub CLI with the `gh skill` subcommand. `skills-bump` also needs `gh auth login`.
- To opt out, delete `agent-skills.lock.json`, or set `rust-toolchains.skills.enable = false;`.

## Keep the devshell banner current

Entering the devshell prints a box of common commands, titled with the crate name. When the project gains a CLI, script or workflow command someone will run often, add it to `rust-toolchains.banner.commands` in `flake.nix`:

```nix
rust-toolchains.banner.commands = [
  {
    command = "cargo run -- serve";
    description = "Start the server";
  }
];
```

Remove entries when the command they describe goes away.

## Run the checks before you finish

In this project every hook runs in both the pre-commit and pre-push stages, so `prek run --all-files` runs all of them:

- rustfmt, clippy and alejandra
- the tests, through `cargo-test-auto`, which runs `cargo nextest run` when `.config/nextest.toml` exists and `cargo test` otherwise
- `cargo-unused-deps`, which fails on dependencies flagged by Cargo's `unused_dependencies` lint, and on any other warning
- `cargo-unused-dev-deps`, which runs cargo-shear for the dev-dependencies that lint skips. To keep a dev-dependency that cargo-shear wrongly reports as unused, add it to `[package.metadata.cargo-shear] ignored = ["crate-name"]` in `Cargo.toml`.
- `cargo-deny-check`, which checks advisories, licenses, banned crates and sources. To change its policy, set `rust-toolchains.cargoDeny.settings` in `flake.nix`, or add a `deny.toml`, which takes precedence.
- actionlint, on the GitHub Actions workflows
- committed, on the commit messages, as described below

`nix flake check` runs fmt, clippy and the tests in the Nix sandbox with the same toolchain, building from `vendor/`. `nix fmt` formats every file.

git hooks do not fire on jj commands, so run the prek commands yourself. `prek.toml` is generated on shell entry and gitignored. To change hooks, edit `rust-toolchains.prek.hooks` in `flake.nix`, never `prek.toml`.

`jj fix` formats Rust and Nix files with rustfmt and alejandra.

## Commit the lockfile

`Cargo.lock` belongs in version control. `packages.default` and the flake checks read it, so `nix build` and `nix flake check` fail until it exists and is tracked.

## Write commit messages as conventional commits

Every commit message follows [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). [git-cliff](https://git-cliff.org) turns the titles into `CHANGELOG.md` and each GitHub release's notes, so the title of a `feat`, `fix`, `perf` or `revert` commit is what users read.

```
<type>[!]: <description>

<body explaining why, wrapped at 72 columns>

[Changelog: skip]
```

Pick the type by what a user of the `little_boxes` binary would notice:

| Type | Use it for | In the changelog |
|---|---|---|
| `feat` | A new flag, option or behaviour | Added |
| `fix` | A bug fix, or a change to existing behaviour | Fixed |
| `perf` | A speed or memory improvement users can notice | Performance |
| `revert` | Undoing an earlier commit | Reverted |
| `refactor` | A code change with no visible effect | Left out |
| `test` | Tests only | Left out |
| `docs` | Documentation only, including this file | Left out |
| `ci` | GitHub Actions workflows and release tooling | Left out |
| `build` | Cargo, Nix or vendored dependency changes | Left out |
| `chore` | Anything else, including release commits | Left out |
| `style` | Formatting only | Left out |

Write the title so it works as a changelog entry:

- Keep the whole title within 72 characters, including the type.
- Start the description with a lowercase imperative verb, and end it without a full stop: `fix: exit quietly when stdout is closed early`.
- Name what the user sees, such as the flag, the input or the output, not the function you changed.

Mark a breaking change with `!` after the type, as in `feat!: remove --legacy-mode`. git-cliff bumps the major version for it.

When a `feat` or `fix` commit changes something users never had, such as a fix to a bug introduced earlier in the same unreleased batch, add the trailer `Changelog: skip` as the last line of the body. git-cliff leaves it out of the changelog.

Never edit `CHANGELOG.md` by hand. To change what it will say, reword the commit with `jj desc -r <change>` before it is pushed.

## Check commit messages before pushing

The [committed](https://github.com/crate-ci/committed) linter enforces the format, using the rules in `committed.toml`. It runs in two places:

- **Locally, as a prek hook.** `prek run -a` runs it along with the other hooks, and lints every commit after the 1.12.1 release up to `@-`. jj does not run git hooks, so push with `jj push`, which runs `prek run -a` before `jj git push`. If `jj push` is not defined, run `prek run -a` yourself and push only if it passes.
- **In CI.** The Commits workflow lints the same range on each push and pull request.

To preview the changelog entries your unreleased commits will produce:

```sh
git-cliff --unreleased
```

## Release with the runbook

`RELEASING.md` is the release runbook. Follow it in order and do not skip steps.

## Contribute tooling improvements back

When you improve tooling that is not specific to this project, such as hooks, devshell packages, checks, lints or flake structure, it probably belongs in rust-toolchains so every project gets it.

1. Ask the user whether to send the improvement upstream. Describe it in one or two sentences.

2. If the user approves, send it with `agent-dms`. Name this project, say what changed and why, and include the relevant diff or file contents:

   ```sh
   agent-dms send rust-toolchains -s "Add cargo-deny check to the flake module" <<'EOF'
   From little_boxes: ...
   EOF
   ```

3. Keep the local change until rust-toolchains ships it, then remove the local copy and run `nix flake update rust-toolchains`.

Never send anything without the user's approval. If `agent-dms` is not available, tell the user what you would have sent instead.

# little_boxes

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

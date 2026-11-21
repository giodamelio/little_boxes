# Crate naming

Deep material for [SKILL.md](../SKILL.md). Read it when you name a new crate or rename one.

## Naming conventions

- When a workspace hosts several independent stacks, give each stack one prefix:
  `<stack>-<role>`. The prefix makes a cross-stack dependency visible in a diff.
- Name test-support and utility crates by role, with no stack prefix:
  `golden-test-support`, `network-fixture`.
- Do not encode a layer number in the name. Layers move during a refactor;
  renaming a crate is expensive.
- Cargo maps hyphens in a package name to underscores in the library target, the
  `use` path, and the artifact file name. Renaming a package renames
  `lib<name>.so` or `lib<name>.a` and breaks every loader and binding that names
  it. To rename the package and keep the artifact and the `use` path, set
  `[lib] name = "<old_name>"`. The `[workspace.dependencies]` key, each
  dependent's dependency key, and every `-p <name>` in scripts and CI still
  change.

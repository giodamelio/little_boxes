# Direction rules: examples and background

SKILL.md states each rule. This file holds the examples and the background for a rule whose
application is not obvious.

- [1. No upward dependencies](#1-no-upward-dependencies)
- [2. Independent stacks stay independent](#2-independent-stacks-stay-independent)
- [3. Test-support crates are dev-dependencies only](#3-test-support-crates-are-dev-dependencies-only)
- [4. Observation is an adapter edge](#4-observation-is-an-adapter-edge)
- [5. Platform isolation uses `cfg`](#5-platform-isolation-uses-cfg)

## 1. No upward dependencies

Cargo rejects a cycle of normal or build edges: "cyclic package dependency: package `<name>
v<version> (<path>)` depends on itself". Cargo accepts an upward edge that closes no cycle. That
edge is still an architecture defect: it drags I/O, platform code, and heavy dependency trees
into crates that were supposed to stay pure. Cargo also accepts a cycle that closes through a
dev-dependency; rule 3 allows that cycle for a test-support crate.

```toml
# crates/core-config/Cargo.toml   (Layer 0)
[dependencies]
runtime-engine = { workspace = true }   # WRONG: Layer 0 -> Layer 2
```

Fix: find the type that `core-config` actually needs, and move that type down into Layer 0. The
runtime crate then depends on the config crate, not the reverse.

## 2. Independent stacks stay independent

```text
Stack A:  ffi-a -> runtime-a -> domain-a -> core-config   -> core-types
Stack B:  ffi-b -> runtime-b -> domain-b -> device-config -> core-types
```

An arrow points from a crate to its dependency. The stacks meet only at the shared Layer 0 crate
`core-types`. `runtime-b` must not depend on `runtime-a`: a cross-stack edge means one stack can
no longer be built, tested, or shipped alone.

## 3. Test-support crates are dev-dependencies only

Golden-file, network-fixture, soak, and platform-logging harness crates pull in servers,
temporary files, and assertion machinery that must never reach a shipped artifact.

A test-support crate often depends on the crate it tests. That dev-dependency cycle compiles,
but a unit test in `src/` then sees two copies of the crate under test. The build fails with
E0308 "expected `Token`, found `my_crate::Token`" and the note "there are multiple different
versions of crate `my_crate` in the dependency graph". An integration test under `tests/` links
the one library copy that the support crate also uses.

## 4. Observation is an adapter edge

```text
ALLOWED:   diagnostics-monitor -> runtime-engine -> domain
FORBIDDEN: runtime-engine -> diagnostics-monitor
```

Split a large diagnostics area into a contracts crate, a runner crate, and one crate per protocol
or subsystem, so a consumer takes only the part it needs. The `rust-observability` skill covers
the sink itself: only the host calls `tracing::subscriber::set_global_default`, once.

## 5. Platform isolation uses `cfg`

```toml
# Acceptable: a platform binding crate pulled in only on that target.
[target.'cfg(target_os = "android")'.dependencies]
jni = { workspace = true }
```

```toml
# WRONG: a target gate used to smuggle an upward dependency into a domain crate.
[target.'cfg(target_os = "android")'.dependencies]
ffi-adapter = { workspace = true }
```

A host-only `cargo tree` does not show the second edge; the `cargo metadata` edge list in
SKILL.md does.

# Dependency Direction Reference

Deep material for [SKILL.md](../SKILL.md): how to audit a workspace graph, how
to break an upward edge, and how to split, merge, or delete a crate without an
unreviewable diff.

Contents:

- [Audit procedure](#audit-procedure)
- [Breaking an upward edge](#breaking-an-upward-edge)
- [Splitting a crate](#splitting-a-crate)
- [Merging crates](#merging-crates)
- [Deleting a crate](#deleting-a-crate)
- [Features and direction](#features-and-direction)
- [Granularity](#granularity)
- [Cross-stack sharing decision table](#cross-stack-sharing-decision-table)

## Audit procedure

Run this when you inherit a workspace, or when a change adds an edge you are not
sure about.

### 1. Build the inventory

```bash
cargo metadata --locked --no-deps --format-version 1
```

This lists every workspace member with its manifest path, version, features, and
targets. Nothing else is authoritative. A diagram, a README, and a comment are
all evidence of the past.

Without jq, print the edge list of SKILL.md with Python:

```bash
cargo metadata --locked --no-deps --format-version 1 | python3 -c '
import json, sys
for p in json.load(sys.stdin)["packages"]:
    for d in p["dependencies"]:
        if d.get("path"):
            print("%s -> %s [%s]" % (p["name"], d["name"], d["kind"] or "normal"))' | sort
```

### 2. Assign one layer to every crate

Write the table down. One row per crate, one layer per crate. A crate you cannot
place has usually taken two jobs and needs a split.

Use these questions in order:

1. Does the crate perform I/O, own a task runtime, or touch the clock? It is
   Layer 2 or higher.
2. Does the crate speak to a platform, expose `extern "C"`, or build as
   `cdylib`, `staticlib`, or a binary? It is Layer 3.
3. Does the crate hold decisions or algorithms and no I/O? Layer 1.
4. Does the crate hold only types, errors, and format primitives? Layer 0.

### 3. Check every edge

Compare the workspace edge list from
[SKILL.md](../SKILL.md#read-the-graph-from-cargo) with the table. It lists every
direct edge between path crates, including optional and target-gated ones. Then
read the forward tree of each crate, which adds the transitive and third-party
crates:

```bash
cargo tree --locked -p <crate> -e normal,build --target all
```

Include build-dependencies: build scripts can compile native code, read the
environment, and introduce the same upward coupling as a normal dependency.
Exclude dev-dependencies from the production direction check because a test can
legitimately depend on a higher-layer harness. Keep `--target all`: without it,
`cargo tree` resolves for the host only and hides every platform-gated edge.

Then check the reverse direction for the crates that must stay shared:

```bash
cargo tree --locked --workspace -i <foundation-crate> --target all
```

A foundation crate should have many dependents. An adapter crate should have
none. A crate with exactly one dependent is a candidate to merge into that
dependent.

### 4. Check the dev-dependency edges separately

```bash
cargo tree --locked -p <crate> -e dev
```

Confirm that every fixture and harness crate appears here and never in the
normal tree. A test-support crate that reached the normal tree ships its
servers, temporary directories, and assertion machinery inside the product.

## Breaking an upward edge

An upward edge always means the same thing: a lower crate needs something that
currently lives too high. There are three fixes. Pick by what the lower crate
needs.

### Fix A: the lower crate needs a type

Move the type down. This is the common case and the cheapest fix.

1. Find the smallest crate that both sides already depend on. If none exists,
   create a Layer 0 crate for the type.
2. Move the type definition, its constructors, its `impl` blocks, and its unit
   tests. Move the whole type, not part of it. A type split across two crates
   needs a conversion in both directions and always drifts.
3. Delete the upward dependency from the lower crate.
4. Add the new dependency to both former sides.
5. Run `cargo tree --locked -p <lower-crate> -e normal,build --target all` and
   confirm that the higher crate is gone from it. Only the lower crate's tree
   shrinks: the higher crate keeps its edge to the lower crate and gains the
   edge to the crate that now holds the type.

Do not move a type that carries I/O, a handle, or a runtime type in a field. It
is not a Layer 0 type; you need Fix B.

### Fix B: the lower crate needs behavior

Invert the edge with a trait. Define the trait in the lower crate. Implement it
in the higher crate. Pass the implementation in.

```rust
// Layer 1: domain crate. Defines what it needs, does not know who provides it.
pub trait Clock {
    fn now_millis(&self) -> u64;
}

pub fn expire(deadline_millis: u64, clock: &dyn Clock) -> bool {
    clock.now_millis() >= deadline_millis
}
```

```rust
// Layer 2: runtime crate. Depends downward on the domain crate and supplies it.
pub struct SystemClock;

impl domain::Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        // A wall clock set before 1970 reads as 0 instead of panicking.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}
```

The domain crate now tests with a fake clock and never links the runtime crate.
Use the same shape for a metrics sink, an event emitter, a storage handle, and a
progress callback.

Choose the dispatch form deliberately:

- A generic parameter (`fn expire<C: Clock>(..., clock: &C)`) costs no vtable and
  monomorphizes per implementation.
- A `&dyn Trait` keeps the caller's code size down and lets a struct hold a
  boxed sink chosen at run time.
- Do not add a generic parameter that every caller instantiates with one type.
  The `rust-discipline` skill covers the signature trade-offs.

### Fix C: the two crates are one crate

If the lower crate needs the higher crate's behavior at every call site, and the
higher crate needs the lower one everywhere, the boundary is fiction. Merge
them, and delete the layer that was never enforced. A merged crate that is
honest about its dependencies is better than two crates that lie about theirs.

## Splitting a crate

Split when the crate has taken two jobs, or when a stable half is rebuilt every
time an unstable half changes.

Procedure that keeps the diff reviewable:

1. **Cut along the module boundary that already exists.** If no module boundary
   matches the split, do the module refactor first, in its own commit.
2. **Create the new crate** with the
   [checklist in SKILL.md](../SKILL.md#creating-a-new-crate). The new crate goes
   at the layer of the extracted code, which is usually lower than the original.
3. **Move files unchanged.** Use `git mv` so the history follows. Do not rename,
   reformat, or fix anything while moving. A move commit with no content change
   is readable; a move commit with edits is not. Move each inherent impl and
   each impl of a foreign trait with its type, also when it lives in another
   file: the orphan rule rejects it in the old crate with E0116 or E0117. An
   impl of a trait that the old crate defines may stay.
4. **Fix visibility.** Items that were `pub(crate)` and are now used across the
   crate boundary become `pub`. Review each one: this is a new public API, and
   it is permanent in practice. Do not blanket-`pub` a module to make the build
   pass.
5. **Add a re-export shim in the original crate** so dependents keep compiling:

   ```rust
   // Temporary: keeps existing paths working for one release.
   pub use new_crate::{Request, Response};
   ```

6. **Update dependents in a follow-up commit**, then delete the shim. Keep the
   shim's lifetime short and stated in the comment. A shim that survives a year
   becomes a second public API.
7. **Verify** the split actually cut the dependency tree:

   ```bash
   cargo tree --locked -p <new-crate> -e normal
   cargo tree --locked -p <original-crate> -e normal
   ```

   If the new crate's tree is not smaller than the original's, the split bought
   nothing. Undo it.

## Merging crates

Merge when a crate has exactly one dependent, changes only together with that
dependent, and exposes no separate unsafe policy, feature set, or target set.

1. Move the modules into the dependent with `git mv`.
2. Demote every item that is no longer used outside to `pub(crate)` or private.
   This is the value of the merge: the public surface shrinks.
3. Remove the crate from `members` and from `[workspace.dependencies]`.
4. Remove the dependency line from every former dependent.
5. Run `cargo metadata --locked --no-deps` and confirm the member is gone.
6. Run `cargo update --workspace`, and commit `Cargo.lock`. A `--locked` build
   succeeds with the stale `[[package]]` entry of a removed member still in the
   lockfile; only an update removes it.

## Deleting a crate

A crate that nothing depends on still costs build time and review attention.

```bash
# Must print nothing. The edge list includes dev, build, optional, and
# target-gated edges. cargo tree -i does not: it resolves only default features.
cargo metadata --locked --no-deps --format-version 1 \
  | jq -r '.packages[] | .name as $n | .dependencies[] | select(.path != null)
           | "\($n) -> \(.name) [\(.kind // "normal")]"' | grep -- '-> <crate> '
```

Without `jq`, pipe the same `cargo metadata` output through the `python3` filter
in [Read the graph from cargo](../SKILL.md#read-the-graph-from-cargo), then
through `grep -- '-> <crate> '`.

Then remove, in one commit: the directory, the `members` entry, the
`[workspace.dependencies]` entry, any dependency line in other members, any
entry in a supply-chain policy file that names it, and any build-system
reference to its artifact. Run `cargo update --workspace` to drop the stale
lockfile entry, and commit `Cargo.lock` with the change.

Do not leave a crate in `members` with an empty `lib.rs` as a placeholder. It
compiles, it lints, it slows every build, and it tells the next reader that the
area is still alive.

## Features and direction

A feature that adds a dependency is still a dependency, and the direction rules
apply to it.

```toml
# WRONG: an optional upward dependency is an upward dependency.
[dependencies]
runtime-engine = { workspace = true, optional = true }

[features]
with-runtime = ["dep:runtime-engine"]
```

Rules:

- A crate must not gain a higher-layer dependency behind a feature. Move the
  code that needs the feature up one layer instead.
- Features are additive. When one member of a build enables a feature on a
  shared dependency, every other member in that build sees the dependency with
  the feature on. Resolver versions 2 and 3 stop that unification from crossing
  into build-dependencies, proc-macro dependencies, and target-specific
  dependencies that do not apply to the current target, but they do not separate
  two normal dependents of the same crate. Do not rely on a feature being off in one
  crate.
- A `default` feature that pulls in I/O turns a Layer 0 crate into a Layer 2
  crate for every consumer that forgets `default-features = false`. Keep
  foundation crates feature-light, and keep the I/O in a separate crate.
- Check what a feature actually did:

  ```bash
  cargo tree --locked -p <crate> -e normal --features <feature>
  ```

The `cargo-workflows` skill covers feature unification and lockfile effects;
`cargo tree --locked -e features -i <dep>` shows which crate enables a feature.

## Granularity

More crates give better incremental rebuilds and better parallel compilation,
because cargo can compile independent crates at once and skip a crate whose
inputs did not change. More crates also cost more manifests, more public API
surface, and more places to look.

Guidance:

- Split a crate that is large and stable away from code that changes every day.
  The stable half stops being rebuilt.
- Do not split a crate into pieces that always change together. You pay every
  cost and gain no rebuild.
- Generic-heavy code monomorphizes in the consumer, so moving it to its own
  crate does not remove that work from the consumer's rebuild. Split for API
  clarity there, not for build time.
- Measure before and after with `cargo build --locked --timings`; the
  `rust-performance` skill covers the measurement discipline. Reason about a
  compile-time claim only with a report in front of you.

## Cross-stack sharing decision table

Use when two independent stacks appear to need the same code.

| What is shared | Where it goes |
|----------------|---------------|
| A plain data type or an error enum | A Layer 0 crate both stacks depend on |
| A parsing or encoding routine with no I/O | A Layer 0 crate |
| A pure algorithm with no platform assumption | A Layer 1 crate that neither stack owns |
| A behavior that needs a runtime handle | Nowhere shared. Define a trait in Layer 0 and implement it once per stack |
| A whole subsystem | Nothing. Copy the small part you need. A shared subsystem couples the release of both stacks |

The last row is deliberate. Two independent stacks that share a subsystem stop
being independent: a fix in one blocks the other's release, and a test failure
belongs to nobody. Duplication of a small routine is cheaper than a coupling you
cannot remove later.

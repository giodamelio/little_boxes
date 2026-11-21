# Unwrap audit and typed-error migration

Use this file when a workspace already has a large `.unwrap()` and `.expect()` population and
you must reduce the risk without a rewrite. The general `unwrap` and `expect` rule lives in the
`rust-discipline` skill; this file covers FFI-reachable code.

Contents:

- Step 1: measure
- Step 2: rank by reachability
- Step 3: replace by category
- Step 4: roll out the lints
- Step 5: the `#[expect]` contract
- Typed errors on an FFI path: the typed public error and its single mapping site
- Audit report shape

## Step 1: measure, do not remember

```bash
# Total in production code.
rg -n --type rust '\.unwrap\(\)|\.expect\(' \
  -g '!target/**' -g '!**/tests/**' -g '!**/benches/**' | wc -l

# Per file, worst first.
rg -c --type rust '\.unwrap\(\)|\.expect\(' \
  -g '!target/**' -g '!**/tests/**' -g '!**/benches/**' \
  | sort -t: -k2 -rn | head -20

# Directories that define extern functions.
rg -l 'extern ("(C|system)"|fn)' --type rust | xargs -n1 dirname | sort -u

# Crates that the boundary crate reaches: the rank 2 sites live here.
cargo tree -p <boundary-crate> -e normal --prefix none
```

Record the number in the pull request, not in the skill. The number changes; the method does
not.

## Step 2: rank by reachability, not by count

A crate with 400 unwraps that no foreign caller reaches is lower risk than a crate with 12
unwraps inside an entry point. Rank the sites:

| Rank | Site | Why |
|---|---|---|
| 1 | Inside an `extern` body or a callback that a foreign runtime invokes, outside the guard | Aborts the host with `panic in a function that cannot unwind` (undefined behaviour below MSRV 1.81) |
| 2 | On a code path an entry point calls, with input-derived data | Turns any malformed input into a crash |
| 3 | In a `Drop` implementation | Double panic during unwinding aborts |
| 4 | In a spawned task or thread | Kills the task; may deadlock the waiter |
| 5 | In startup code that runs once | Fails loudly and early; lowest risk |

Fix rank 1 to 3 first. Leave rank 5 alone unless it is cheap.

## Step 3: replace by category

### `Result` with an error type

Replace `serde_json::from_str::<Config>(raw).unwrap()` with a mapped error:

```rust
let config: Config = serde_json::from_str(raw).map_err(Error::InvalidConfig)?;
```

### `Option` from input

Replace `url.host_str().unwrap()` with:

```rust
let host = url.host_str().ok_or(Error::MissingHost)?;
```

### Indexing and slicing

Replace `&buf[..HEADER_LEN]` with:

```rust
let header = buf.get(..HEADER_LEN).ok_or(Error::Truncated)?;
```

### Numeric conversion

`buf.len() as u32` truncates silently. Replace it with:

```rust
let len = u32::try_from(buf.len()).map_err(|_| Error::TooLarge)?;
```

Keep a panicking conversion only when construction already bounds the value. Check the bound
in the same function with a check that survives release builds, and state the invariant in the
`expect` message:

```rust
assert!(buf.len() <= MAX_U32 as usize, "buffer length is bounded by construction");
let len: u32 = buf.len().try_into().expect("buf.len() <= MAX_U32 is checked above");
```

Use the `map_err` form when the length comes from input.

### Arithmetic

Integer overflow panics in debug builds and wraps in release builds unless the profile sets
`overflow-checks = true`. A wrapped length is worse than a panic, because it becomes a bad
slice bound.

```rust
let end = offset.checked_add(len).ok_or(Error::Overflow)?;
```

Turn on `overflow-checks = true` in the release profile of any crate that parses untrusted
input. You trade a small cost for a caught bug.

### Allocation

`Vec::with_capacity(n)` with an attacker-controlled `n` aborts when the allocator fails, and
no guard catches an abort.

```rust
let mut buf: Vec<u8> = Vec::new();
buf.try_reserve(len).map_err(|_| Error::TooLarge)?;
```

Bound the length before you allocate. `try_reserve` is the second line of defence, not the
first.

### Lock acquisition

State the poison policy at each `lock()`; the `rust-discipline` skill owns it. When a panic under
the lock must also stop the next user:

```rust
let state = self.state.lock().expect("state mutex poisoned");
```

When the data is still valid after a panic, say why and recover:

```rust
// The map is only ever inserted into, so a partial insert cannot corrupt it.
let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
```

### Environment and configuration at startup

Replace `std::env::var("APP_DATA_DIR").unwrap()` with:

```rust
let path = std::env::var("APP_DATA_DIR").map_err(|_| Error::MissingEnv("APP_DATA_DIR"))?;
```

A library must never read the environment and panic. An application may fail fast, but it
should still print a usable message.

## Step 4: roll out the lints

Deny in one crate at a time. A workspace-wide `deny` on day one produces hundreds of
findings and gets reverted. The `rust-lints` skill owns the workspace floor in the root
`[workspace.lints]` tables; this step adds the panic lints for crates on an FFI path.

Order:

1. `clippy::panic`, `clippy::todo`, `clippy::unimplemented` — usually a small population, and
   each hit is a real defect.
2. `clippy::unwrap_used` on the FFI adapter crates.
3. `clippy::expect_used` as `warn` on the same crates.
4. `clippy::indexing_slicing` and `clippy::arithmetic_side_effects` on parsers and decoders.
5. `clippy::panic_in_result_fn` on the same crates. A function that returns `Result` and still
   panics defeats its own signature.
6. `clippy::missing_panics_doc` on public APIs that keep a documented panic.

Keep the `[workspace.lints]` floor from the `rust-lints` skill unchanged. Every member inherits it:

```toml
# Member Cargo.toml: inherit the floor unchanged.
[lints]
workspace = true
```

A crate on an FFI path keeps `[lints] workspace = true` and tightens the panic lints with
crate-root attributes. Source attributes override the Cargo lint levels, and the crate keeps
the whole floor, including the unsafe lints that an FFI adapter needs most.

```rust
// lib.rs of a crate on an FFI path. Its Cargo.toml keeps `[lints] workspace = true`.
#![deny(
    clippy::unwrap_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::panic_in_result_fn
)]
#![warn(clippy::expect_used, clippy::indexing_slicing, clippy::missing_panics_doc)]
```

Cargo rejects a manifest that sets `lints.workspace = true` and a `[lints.<tool>]` table at
the same time: `cannot override 'workspace.lints' in 'lints'`. If you use a local table
instead, it must restate every `[workspace.lints.rust]` and `[workspace.lints.clippy]` entry.

```toml
# clippy.toml at the workspace root
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
```

`panic_in_result_fn` has no `clippy.toml` test exemption. In a crate that denies it, write
tests that return `()`, or put this attribute on each test module that holds a
`Result`-returning test: `#[expect(clippy::panic_in_result_fn, reason = "test assertions")]`.

Verify with the same command the CI uses:

```bash
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Add `--all-features` only when the features are additive. Otherwise run each supported feature
set; the `cargo-workflows` skill owns that rule. With cargo-hack installed,
`cargo hack check --locked -p <crate> --each-feature --exclude-all-features` runs one feature at
a time.

## Step 5: the `#[expect]` contract

Under `-D warnings`, a kept `.expect("<invariant>")` in a crate that sets `expect_used = "warn"`
needs a scoped suppression. Use `#[expect]` with a reason, not `#[allow]`: `#[expect]` fails
with `unfulfilled_lint_expectations` when the call goes away, so a stale suppression cannot
stay. `#[expect]` needs Rust 1.81. Below that MSRV, use `#[allow(lint)]` with a comment that
states the invariant.

```rust
#[expect(clippy::expect_used, reason = "the key is inserted above, so the lookup cannot miss")]
let entry = map.get(&key).expect("key inserted above");
```

Rules:

- Never put `#![allow(clippy::unwrap_used)]` at a crate root to silence a migration. That
  deletes the signal for every future line.
- Scope the suppression to the smallest item: the statement or the function, not the module.
- A suppression with no `reason` is a review failure, the same as a bare `.unwrap()`.
- Re-check the reasons when the surrounding code changes. The proof is attached to the code
  around it, and that code moves.

## Typed errors on an FFI path

The `rust-code-style` skill owns the `thiserror` and `anyhow` split. For a crate on an FFI path:

- Give the crate a typed public error. `anyhow::Result` or `Result<_, String>` in its public
  API forces the boundary to match on strings.
- Map the typed error to the foreign representation in exactly one place per boundary crate.
  A second mapping site drifts.
- Design the error type so the boundary can map it without a string match. Use one error enum
  per crate, not one per function. A caller that must match on six unrelated enums starts to
  stringify.
- Mark it `#[non_exhaustive]` when the crate is a public dependency, so a new variant is not a
  breaking change.
- Keep the `Display` text short and free of secrets. It can reach a host log, or a Java
  exception message through a `jni` error policy.
- Do not put a backtrace in the error type. The shipped panic hook emits only a closed site code
  plus bounded numeric location. Reproduce locally with `RUST_BACKTRACE=full`, or symbolicate
  the crash artifact offline against the exact binary.
- Convert an `anyhow::Error` to a typed error before the value crosses into a crate that a
  binding layer consumes. A downcast at the FFI boundary shows the type was wrong two layers
  earlier.

## Audit report shape

When you finish an audit, report these lines and nothing else:

1. Total `.unwrap()`/`.expect()` in production code, before and after.
2. The count on FFI-reachable paths, before and after.
3. Crates that now deny `clippy::unwrap_used`.
4. Every remaining rank 1 to 3 site, with the reason it stays.
5. The lint command CI runs, so the next reviewer can reproduce the number.

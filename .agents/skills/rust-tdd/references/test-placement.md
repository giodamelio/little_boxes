# Test Placement and Old Toolchains

Extended reference for the `rust-tdd` skill. Read this when you choose where a new test lives,
when a test runs a binary of the package, or when a run uses a toolchain older than Rust 1.88.

## Test placement

Pick the cheapest layer that can observe the behavior. Escalate only when the behavior
needs real I/O, a real peer, or a real host runtime.

| What you test | Location | Command |
|---|---|---|
| Internal logic of one module | `#[cfg(test)] mod tests` in the same file | `cargo nextest run --locked -p <crate> <name>` |
| Public API of one crate | `tests/*.rs` in that crate | `cargo nextest run --locked -p <crate> --test <file>` |
| Documented usage of the public API | `///` doc comment examples | `cargo test --locked --doc -p <crate>` |
| Command-line behavior of a binary | `tests/*.rs` that runs `env!("CARGO_BIN_EXE_<name>")` | `cargo nextest run --locked -p <crate> --test <file>` |
| Wiring between crates | integration tests of the crate that owns the composition | `cargo nextest run --locked --workspace` |
| FFI boundary behavior | tests inside the boundary crate, plus one test on the host-language side | boundary crate tests, then the host test runner |
| Network or process end-to-end | `tests/*.rs` with `#[ignore = "needs network"]`, or behind a cargo feature | `cargo nextest run --locked -p <crate> --run-ignored all`; libtest: `cargo test --locked -p <crate> -- --include-ignored` |

Unit tests in `#[cfg(test)] mod tests` can reach private items. Integration tests in
`tests/` can reach only the public API, so they catch API design problems that unit tests
hide. Write the public-API test when the behavior is part of the contract. Do not make an
item `pub` only for a test.

## Tests that run a binary

Cargo builds each binary of the package before it compiles the integration tests, and sets
`CARGO_BIN_EXE_<name>` to the path of the binary. Do not hard-code `target/debug/<name>`. That
path changes with `--target`, `--profile`, and `CARGO_TARGET_DIR`. Cargo does not build a
binary whose `required-features` are off, so the test fails with `NotFound` or runs a stale
binary from an earlier build. Enable those features for the test run.

## Output capture on old toolchains

Write `--no-capture`. libtest deprecated the `--nocapture` spelling in Rust 1.88. The old
spelling still works without a warning, so copied commands keep it alive. `--no-capture`
needs Rust 1.88 or later; an older libtest rejects it with `Unrecognized option`. On an
older toolchain, such as an MSRV lane, set `RUST_TEST_NOCAPTURE=1`. It works on both sides
of 1.88.

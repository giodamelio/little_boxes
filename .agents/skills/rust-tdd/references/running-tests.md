# Running One Test

Extended reference for the `rust-tdd` skill. Read this when you select tests with a filter or an
exact path, show test output, or resolve the lockfile for a `--locked` run.

## Select tests

Use cargo-nextest when it is installed (`cargo nextest --version` succeeds). It runs each
test in its own process, so an abort, a segfault, or a stack overflow in one test does not
end the run, and process-global state does not leak between tests. Otherwise use
`cargo test`.

| Goal | cargo-nextest | cargo test (libtest) |
|---|---|---|
| Tests whose name contains a substring | `cargo nextest run --locked -p <crate> <substring>` | `cargo test --locked -p <crate> <substring>` |
| One test by its exact path | `cargo nextest run --locked -p <crate> -E 'test(=module::tests::name)'` | `cargo test --locked -p <crate> -- --exact module::tests::name` |
| One integration test file `tests/<file>.rs` | `cargo nextest run --locked -p <crate> --test <file>` | `cargo test --locked -p <crate> --test <file>` |
| Show the output of passing tests | add `--no-capture` (it also runs tests serially) | add `-- --no-capture` |
| Doctests | not supported | `cargo test --locked --doc -p <crate>` |

Read the test count on every filtered run. A filter that matches nothing is a false green:
`cargo test <filter>` prints `0 passed; ... N filtered out` and exits 0. cargo-nextest prints
`error: no tests to run` and exits 4.

Write `--no-capture`, not the deprecated `--nocapture`. Read
[test-placement.md](test-placement.md) when a run uses a toolchain older
than Rust 1.88.

Pass `--locked` to every cargo command that builds or runs code, so that a green run used
the dependency set that CI uses. The `cargo-workflows` skill, when it is installed, owns the
lockfile policy. Otherwise, when `--locked` fails after a manifest edit, resolve the lock
without a build (`cargo metadata --format-version 1 > /dev/null`), vet and commit it, and
rerun with `--locked`.

---
description: Use when writing Rust code test-first (TDD, red-green-refactor), reproducing a bug as a failing test, pinning behavior before a refactor, or reviewing whether tests came first. Also for hand-written fakes with a fault-injection queue, and async tokio tests with paused time. Not for loom, proptest, fuzzing, mutation testing, or golden files; use rust-test-tools.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-tdd
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: a3af0a147ebb5d6ebd857dc83e96517625cd35f4
name: rust-tdd
---
# Rust TDD

One invariant carries the whole method: the test fails first, for the reason you predicted.
A test written after the implementation only shows that the code does what the code does.
Every step below serves that invariant.

## The cycle

Repeat this cycle for each behavior. One test, one behavior, one cycle.

1. **RED** — Write one failing test. Write down the expected failure in one sentence: which
   assertion fails, and with which value. Run the test and compare.
2. **GREEN** — Write the minimum implementation that makes the test pass. Run the test
   again. Add no code that no test requires.
3. **REFACTOR** — Clean up the test and the implementation. Run the tests of the crate.

Run the format and lint gate once before each commit, not after each cycle. The
[Verification](#verification) section has the commands.

### Read the RED result

| Observed result | What it means | Action |
|---|---|---|
| Fails as predicted | The test controls the behavior you target | Go to GREEN |
| Passes | The behavior exists already, or the test asserts nothing | Fix the test, or delete it and pick a real gap |
| 0 tests ran | The filter matched nothing: a typo or a wrong module path | Fix the filter until the run reports exactly 1 test |
| Fails, but not as predicted | You do not understand the code yet | Stop. Read the code. Rewrite the test. |
| Panics with `not yet implemented` | A `todo!()` stub panicked before the assertion ran | Make the stub return a wrong but valid value, such as `Default::default()` |
| Does not compile | Not a RED yet | Add the signature with a stub body, then run again |

The test must call the changed code and assert on its result. A test that drives only a
fake, a wrapper, or a copy of the logic passes whatever the change does. The RED-to-GREEN
transition is the proof that the test depends on the change. Report the RED failure line and
the test count with the change.

## Verification

Run the gate once before each commit:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --locked -p <crate>      # without nextest: cargo test --locked -p <crate>
cargo test --locked --doc -p <crate>       # when the crate has doctests
```

When the change touches an API that other workspace crates use, and always before merge, run
`cargo nextest run --locked --workspace` (without nextest: `cargo test --locked --workspace`)
and `cargo test --locked --workspace --doc`.

- Add `--all-features` only when the workspace features are additive. Otherwise check each
  feature set: `cargo hack check --locked -p <crate> --each-feature --exclude-all-features`.
- For a crate that has only doctests, cargo-nextest finds no test and exits 4. Run
  `cargo test --locked --doc -p <crate>` for that crate.

What a green run does not prove:

| Green result | Does not prove | Check |
|---|---|---|
| A cargo-nextest run | That the doctests pass | `cargo test --locked --doc -p <crate>` |
| A filtered run | That any test ran | Read the test count |
| A cargo-nextest run | That the suite passes under `cargo test`, whose tests share one process: statics, environment, current directory | `cargo test --locked -p <crate>` when tests touch process-global state |
| A `#[should_panic]` test | That the expected panic fired: without `expected`, any panic passes | Pin `expected = "<message substring>"` |
| An async test that spawns tasks | That the tasks did not panic: tokio reports a task panic only through its `JoinHandle` | Await every `JoinHandle` and check its result |
| Any test run | That `unsafe` code has no undefined behavior | The `rust-sanitizers-miri` skill, when it is installed |
| New tests pass | That the tests constrain the change | The RED-to-GREEN transition. For a critical change, `cargo mutants --in-diff <diff-file>`; the `rust-test-tools` skill, when it is installed, has the setup. |

Commit rules:

- Land each test in the same commit as the implementation it drove. Characterization tests
  of existing behavior are the exception: they pass on the current code, so they land first,
  in their own commit.
- Do not delete, weaken, `#[ignore]`, or re-bless a test to make the build green. A failing
  test is information. Fix the code or fix the assertion, and say which one you changed.
- Leave no `todo!()` or `unimplemented!()` stub in the commit.

Read [references/testing-anti-patterns.md](references/testing-anti-patterns.md) when you
review a test suite that somebody else wrote, or when a test is flaky. It has the flaky-test
triage table.

## Run one test at a time

Use cargo-nextest when `cargo nextest --version` succeeds: it runs each test in its own process.
Otherwise use `cargo test`. Read the test count on every filtered run: `cargo test <filter>`
that matches nothing prints `0 passed; ... N filtered out` and exits 0. Pass `--locked` to every
cargo command that builds or runs code. Read [references/running-tests.md](references/running-tests.md)
when you select one test by exact path, show the output of passing tests, or `--locked` fails
after a manifest edit.

## Variants of the cycle

### New feature

Start from the smallest observable behavior of the public API. Add one test per behavior.
Grow the API only when a test needs it.

### Bug fix

1. Write a test that reproduces the bug through the public API. Run it. It must fail with
   the reported symptom.
2. Find the root cause. Fix it at the single place that all callers route through, not at
   the call site named in the report.
3. Run the reproduction test. It must pass.
4. Search for code that repeats the defective logic outside the fixed function. Add a
   reproduction test for each copy you find.

Keep the reproduction test after the fix. It is the regression guard.

### Refactor

A refactor must not change behavior, so it needs no new RED. It needs proof that the
behavior is already pinned.

1. Run the existing tests. They must pass before you change anything.
2. If the tests cover the target code poorly, write characterization tests first. Assert the
   current behavior, including the parts you find ugly. Commit them on their own.
3. Refactor in small steps. Run the tests after each step.
4. If a test needs an edit to keep compiling, the change is not a pure refactor. Split the
   work: the behavior change with its own RED, then the refactor.

For a port whose reference a test can call in-process, add a differential property or fuzz
target that feeds the same input to both, including non-ASCII and invalid UTF-8 input.
The `rust-test-tools` skill, when it is installed, has the setup.

### Keep test design independent of the implementation plan

The test must encode the behavior that the caller needs, not the implementation you have in
mind. When the harness offers an independent context, such as a subagent, give it the behavior
statement, the public API, and the test rules, but not your implementation sketch; let it write
the failing test. Otherwise, write the test from the behavior statement before you plan the
implementation.

## Test placement

Pick the cheapest layer that can observe the behavior: a unit test, then a `tests/*.rs`
integration test, then an end-to-end test. Do not make an item `pub` only for a test. Read
[references/test-placement.md](references/test-placement.md) when you choose the layer for a
new test, or when a test runs a binary of the package (`CARGO_BIN_EXE_<name>`).

## Test names and panic tests

Name the behavior and the condition in `snake_case`, for example
`start_returns_error_when_transport_is_already_running`, not `test_start` (catalog default).

Pin the message of every panic test. `expected` matches a substring of the panic message.

```rust
#[test]
#[should_panic(expected = "capacity must be non-zero")]
fn new_rejects_zero_capacity() {
    Buffer::new(0);
}
```

Enable the pedantic `clippy::should_panic_without_expect` and `clippy::ignore_without_reason` in
the workspace lint table, so the `-D warnings` gate rejects a bare `#[should_panic]` or `#[ignore]`.

## Test doubles: hand-written fakes

Hand-written fakes are the default: a reader sees everything a fake does, and a fake that is
hard to write shows a wrong seam. When the workspace already uses a mocking crate, follow it.

- Define the seam as a trait in the production crate. The fake implements the trait.
- Name the fake `Fake` + trait name, for example `FakeTransport` for `Transport`.
- Record call counts and the last arguments in public fields, so the test can assert them.
- Configure return values and faults before the test runs.
- Put shared fakes in one place: a `#[cfg(test)]` module, `tests/common/mod.rs`, or a
  dev-dependency crate.
- Create a fresh fake in every test, so the result does not depend on the test order.

Assert the observable result first, then the interaction. A call counter alone shows that
the code called something, not that the behavior is right.

## Fault injection for error paths

An `if cfg!(test)` branch in production code is not a test of an error path. Drive failures
from the fake through one typed queue: a `FaultQueue<T>` of `FaultSpec<T>` entries, each with
a target, a `FaultOutcome`, and a `FaultScope`. A `OneShot` fault fires once. A `Persistent`
fault fires on every matching call until you clear it.

Read [references/fault-injection.md](references/fault-injection.md) when you write a fake,
add the queue to a workspace, or pick an outcome or a scope. It has a complete
`FakeTransport`, the queue implementation, the outcome-to-failure-mode table, the scope
rules, and the check that a fault really fired.


Assert the mapped error with `std::assert_matches!` (Rust 1.96; import it), or
`assert!(matches!(..))` below that MSRV. For cancellation and progress faults across an FFI boundary, use the
`ffi-error-progress-cancel` skill, when it is installed; at minimum, assert that a cancel maps to
`Cancelled`, not to a failure.

## Async tests with tokio

Use `#[tokio::test]` for code that runs on tokio. Inside it, never call `Runtime::block_on` or a
blocking wait such as `std::sync::mpsc::Receiver::recv`: use `.await`. Read
[references/async-tests.md](references/async-tests.md) ("Runtime mistakes") when a test panics
with `there is no reactor running` or hangs.

Do not synchronize a test with a sleep: the suite is slow when it passes and flaky when it
fails. Use a channel. For code that waits on a timer, use
`#[tokio::test(start_paused = true)]`, with the tokio `test-util` feature in
`dev-dependencies` only. Read the same reference when a test waits on time, or asserts a state
while an operation is in flight.

## Golden contract tests

A golden test compares output that another party parses with a committed fixture. The
`rust-test-tools` skill, when it is installed, owns golden files and the bless review.

## Related skills

When installed: `rust-test-tools` (proptest, loom, cargo-mutants, cargo-fuzz), `rust-panic-safety`
(containing a panic at a boundary), `rust-debugging` (a test that fails with no visible cause;
start with `RUST_BACKTRACE=1 cargo test --locked -p <crate> <test_name> -- --no-capture`), and
`rust-sanitizers-miri` (undefined behavior that a passing suite does not report).

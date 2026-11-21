---
description: Use when reviewing Rust code that someone else wrote (a diff, a pull request, a file, or a crate) and writing a prioritized review report, with each finding at its location, the rule it breaks, and the smallest fix as code. Not for writing or fixing the code, or for a review of one area that one skill owns, such as unsafe or async code; load that skill.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-review
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 0453bb9dadbee80b0d47a6e3f245fba05a56cb66
name: rust-review
---
# Rust Review

A review reports what to change and why; the author decides what to fix. Do not edit the code
under review unless the request asks for a fix. This skill holds the method and the report. The
rules live in the skills that own them: the table in [Where to look](#where-to-look) names the
owner of each area, and the check to run when that skill is not installed.

## Scope

1. **Read what the request names.** For a diff, read the changed lines and the code they can
   break: callers of a changed signature, invariants of a changed type, and tests of a changed
   behavior. `git diff --stat <base>...HEAD` lists the files; `git diff <base>...HEAD` shows the
   lines. Do not review unchanged code that the diff cannot affect.
2. **Decide library or application.** A crate that other crates use (a `lib.rs`, `publish` not
   set to `false`, or a path dependency of another crate) gets the API rules in full. An
   application gets the simpler rules: concrete types, one error type with context, no
   speculative generics. Say which one you assumed in the report.
3. **Read the edition and the MSRV** (`edition`, `rust-version` in `Cargo.toml`). Do not propose
   an API or a syntax that is newer than the MSRV.
4. **Respect the framework.** A framework convention wins over generic advice: system parameters
   by value in Bevy, a framework prelude glob, an attribute macro that generates `pub` items. At
   most, report it under Consider.

## Run the tools first

`cargo clippy` and `cargo test` run code from the reviewed tree: build scripts, procedural
macros, and the tests themselves. Before you run them on code from someone you do not trust, such
as a pull request from outside the team, read `build.rs`, the proc-macro crates, and the changed
tests, and get the user's approval. Run them in a sandbox with no secrets and no network. Without
that approval, review by reading, and say so in the report.

When the code is trusted, or after approval, run these from the workspace root:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

- Do not run `cargo fmt` without `--check`: it rewrites the code under review.
- When the repository commits no `Cargo.lock`, `--locked` fails before anything builds. Report
  that, then run without `--locked` only if the code is trusted.
- Report each command and its result. Group clippy output by lint, with the lines; do not paste
  it raw. A lint that the project allows on purpose, with a reason, is not a finding.
- If a tool cannot run (no toolchain, no network for a dependency, no approval), say so in the
  report and review by reading.

## Where to look

Work through the areas in this order. The first four are where real defects live. Run a lint
that the table names with `cargo clippy --locked --workspace --all-targets -- -W clippy::<lint>`.

| Area | Look for | Owner of the rule | Check without the owner |
| --- | --- | --- | --- |
| Soundness | `unsafe` with no `// SAFETY:` comment, `transmute`, `unsafe impl Send` or `Sync`, a safe function that can cause undefined behavior | `rust-unsafe` | `clippy::undocumented_unsafe_blocks`; `cargo +nightly miri test` on the tests that reach the code |
| Threads and tasks | A value that is not `Send` across a thread or an `.await`, a lock guard held across `.await`, a `select!` branch that is not cancel-safe | `rust-send-sync`, `rust-async-internals` | `clippy::await_holding_lock`; a compile-time `fn assert_send<T: Send>() {}` call |
| Atomics and globals | `Ordering::Relaxed` that publishes data, `static mut` | `memory-model` | `MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test` |
| Reachable panics | `unwrap`, `expect`, indexing, or an `as` cast on input from outside; a panic that can cross `extern "C"` | `rust-panic-safety` | `clippy::unwrap_used`, `clippy::indexing_slicing`, `clippy::cast_possible_truncation` |
| FFI boundary | An `extern` function, a JNI or UniFFI export, a Swift bridge type: a pointer or length the caller controls, ownership that crosses, an unguarded panic | `rust-jni`, `uniffi-boundary`, `rust-swift-ffi` | `clippy::missing_safety_doc`; a `catch_unwind` guard on each `extern` body (`rust-panic-safety`); ASan on the tests that cross |
| Drop and borrow points | A guard alive through a `match`, a temporary that drops early or late | `rust-borrow-semantics` | `clippy::significant_drop_in_scrutinee`; a `try_lock().is_ok()` assertion in a test |
| Errors | `String`, `()`, or `Box<dyn Error>` as a library's error type, `anyhow::Error` in a public signature, `map_err(\|_\| ...)` that drops the cause | `rust-error-handling` | a `thiserror` enum with `#[from]` or `#[source]` for each cause, so `Error::source` keeps the chain |
| API shape | `&String` or `&Vec<T>` parameters, a `bool` parameter, `Rc<RefCell<_>>` in a `pub` signature, `get_` getters, conversions named against their cost, missing standard traits | `rust-discipline` and its API conventions reference | `clippy::ptr_arg`, `clippy::fn_params_excessive_bools`, `clippy::wrong_self_convention`, `missing_debug_implementations` |
| Published compatibility | A new variant on an exhaustive `pub enum`, a new field on a struct whose fields are all `pub` and that has no `#[non_exhaustive]`, a removed item | `rust-crate-release` | `cargo semver-checks --baseline-rev <base>` |
| Wire formats | A changed `Serialize` or `Deserialize` on stored or sent data | `rust-serde` | a test that decodes a recorded old payload with `serde_json::from_str` |
| Dependencies | A change to `Cargo.toml` or `Cargo.lock`: a new dependency, a new `build.rs`, widened features, a `git` source | `rust-security` | `cargo deny check` or `cargo audit`; a read of the `Cargo.lock` diff |
| Lints and suppressions | `#[allow]` with no reason, `#![deny(warnings)]` in source | `rust-lints` | `clippy::allow_attributes_without_reason` |
| Tests | A bug fix with no regression test, a test that restates a constant | `rust-tdd` | the test count in `cargo test` output before and after the change |
| Ports | A rewrite that must keep another implementation's output | `rust-port` | the recorded comparison in its `PORTING.md` |
| Speed and size | A performance claim | `rust-performance` | a profile or `cargo bench` numbers; without them, the claim is not a finding |

For a full API pass on a library, read the review checklist reference of the `rust-discipline`
skill when it is installed; otherwise run `clippy::pedantic` once and use the API shape row above.

## Severity

| Section | What belongs there |
| --- | --- |
| Must fix | Undefined behavior or unsoundness, a data race, a panic or a hang that outside input can reach, data loss, a security hole, a breaking change to a published API |
| Should fix | Code that works but breaks a rule at a real cost: an error that loses its cause, an API that is hard to use correctly, a bug fix without a test |
| Consider | Style, naming, and polish; a deliberate choice that has a cost worth naming |

A Must fix needs evidence: a failing test, a tool line, or the input that triggers it. Without
evidence, put it under Should fix and mark it "not verified".

## The report

```text
# Review: <what was reviewed>

<One paragraph: what the code does, library or application, the verdict, and the one thing to
fix first. List the commands that ran and their results, or say that none could run.>

## Must fix

### 1. <Short title> (`src/file.rs:42`)
Rule: <owner skill, and the rule code where one exists: C-GETTER, clippy::ptr_arg, E0502>
Problem: <what goes wrong, and for which input or caller>
Fix:
<the smallest change, as a code block of the lines that change>

## Should fix
### 2. ...

## Consider
### 3. ...

## Already good
- <two to four specific things the code does well>
```

- Give every finding a location (`file:line`, or "whole crate" for a missing test suite) and a
  rule that a reader can look up.
- Show the fix as code, kept to the lines that change. Do not rewrite whole functions.
- Group repeats: one finding for "four `unwrap()` calls on parsed input", with all the lines.
- Keep it in proportion: five to ten findings for one file. Do not bury a Must fix under style
  notes, and do not pad the list.
- When code looks deliberate (a comment explains it, or a test pins it), name the trade-off under
  Consider instead of calling it a defect.
- "Already good" is required. It tells the author what to keep, and it shows that the reviewer
  read the code.

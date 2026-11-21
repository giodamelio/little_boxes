---
description: Use when guarding an extern "C", extern "system", or JNI entry point with catch_unwind, choosing panic = "unwind" or "abort", installing a panic hook, disposing of a panic payload, fixing an UnwindSafe bound with AssertUnwindSafe, or triaging an abort such as "panic in a function that cannot unwind". Also use when a panic can escape a foreign callback, spawned task, thread, or Drop, or when replacing unwrap or expect on an FFI-reachable path. Owns the panic policy at a C ABI boundary.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-panic-safety
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 5c551d42fa744fe4b26ca76112da131d852a76ec
name: rust-panic-safety
---
# Rust panic safety

## Find the boundaries

```bash
# Every function that foreign code can call. A bare `extern fn` is `extern "C"`.
rg -n 'extern "(C|system|C-unwind|system-unwind)"|extern fn|#\[jni_mangle|native_method!' --type rust

# Every symbol that leaves the crate unmangled.
rg -n '#\[unsafe\(no_mangle\)\]|#\[no_mangle\]|#\[export_name|#\[unsafe\(export_name' --type rust

# Every guard that already exists.
rg -n 'catch_unwind|AssertUnwindSafe|with_env' --type rust

# Every panic strategy declared in the tree: manifests, config files, and rustflags.
rg -n --hidden 'panic\s*=\s*"?(abort|unwind)' \
  -g '**/Cargo.toml' -g '**/.cargo/config.toml' -g '**/.cargo/config'
```

Compare lists 1 and 3 from this checkout. `native_method!` generates a `with_env` guard
unless it is `raw` or sets `catch_unwind = false`. Report an entry with neither a
guard nor a documented abort policy (Rule 1).

## Verify a boundary change

| Claim | Check | What a green result does not prove |
|---|---|---|
| Every entry point has a guard | The inventory above, list 1 against list 3 | That the guard maps every outcome |
| The shipped build unwinds | `cargo rustc --locked -p <boundary-crate> --lib --release --target <shipping-triple> -- --print cfg` prints `panic="unwind"` | That every entry point has a guard |
| A panic returns the reserved code | A Rust test that calls the `extern` function with a forced panic and asserts the reserved code | That the shipped build unwinds |
| No new unguarded panic sites | `cargo clippy --locked --all-targets -- -D warnings` with `-p` for every crate on the FFI path, with the lints below | That each kept `.expect` invariant holds; overflow, panicking std APIs (`split_at`, `RefCell::borrow_mut`), and dependency panics |
| No std-checked precondition fires on tested paths | `cargo test --locked` in the default debug profile | Soundness: run Miri, or ASan where Miri cannot run the foreign code (the `rust-sanitizers-miri` skill) |

Use the shipping `--target`, `--profile`, and environment for `--print cfg`.
Without `--target`, it can print `unwind` for a shipped `abort` build.
Read `references/panic-strategy.md` (section "Check the shipped strategy") for every source it
reflects. When the boundary decodes untrusted input, fuzz the decoder behind it: a guard turns
each panic into a status code and hides it from every other test.

## Done when

- Every inventory entry point has a guard, or a comment that states abort as its policy.
- Every `extern` body is a guard plus a delegation call to a testable plain Rust function.
- The `--print cfg` check prints `panic="unwind"` if any guard uses `catch_unwind`.
- The panic exit differs from every domain error exit.
- The boundary discards the payload through a second guard. Only a run-once loader such as
  `JNI_OnLoad` can leak it instead, with `ManuallyDrop::new`, never `mem::forget`.
- The hook is installed once and emits only a closed site code plus numeric location.
- Every `AssertUnwindSafe` has a comment that names the invariant.
- Every crate on the FFI path denies `clippy::unwrap_used` and `clippy::panic`.
- No `Drop` can panic. Every spawned task or thread reports its panic at a join point or
  through its own guard.

## Failure triage

| Symptom | Likely cause | Action |
|---|---|---|
| `SIGABRT` on Unix, or `0xC0000409` on Windows; stderr shows `panic in a function that cannot unwind` | A panic reached an unguarded `extern "C"` or `extern "system"` function | Add the guard, or document abort as its policy |
| `catch_unwind` never returns `Err`, process still dies | The build uses `panic = "abort"` | Run the `--print cfg` check with the shipping `--target`; fix the source it names |
| `catch_unwind` returns `Ok`, work silently missing | The panic was on another thread or task | Join the handle and inspect `JoinError` |
| `fatal runtime error: Rust cannot catch foreign exceptions, aborting` | A foreign exception reached `catch_unwind` | Catch it on the foreign side |
| `panic in a destructor during cleanup`, then `thread caused non-unwinding panic. aborting.` | A `Drop` implementation panicked while a panic was in flight | Make the `Drop` infallible |
| The boundary catches a panic, then aborts | Dropping the caught payload panicked | Drop the payload inside a second guard; keep a second payload in `ManuallyDrop` |
| `memory allocation of N bytes failed`, then abort | Allocation failure, not a catchable panic | Bound the size; use `Vec::try_reserve` for input-driven buffers |
| `unsafe precondition(s) violated: ...`, then abort, with debug assertions on | An unsafe call broke its precondition: undefined behaviour, not a panic path | Fix the call. Keep `cargo test --locked -p <crate>` in the debug profile: without debug assertions the check is skipped and the UB runs |
| The panic record has no message or backtrace | The shipped privacy contract | Reproduce on the host with `RUST_BACKTRACE=full`, or symbolicate offline |
| Every later call fails with a poison error | An earlier panic poisoned a shared lock | Decide the poison policy; report the original panic |
| `building tests with panic=abort is not supported without -Zpanic_abort_tests` | `-C panic=abort` in a rustflags source reaches the test build | Set the flag only in the shipping build command, or use nightly `-Zpanic-abort-tests` |
| Panic location points into a macro or `core` | An index, a slice range, or an overflow | Reproduce with `overflow-checks = true` in a debug build |

## Silent hazards

A test run rarely shows these defects.

- Treat every panic message as a log line: Android `panic = "abort"` copies it into the
  tombstone, and UniFFI into the foreign exception. Never put secrets or user data in a panic,
  `expect`, or `unreachable!` message.
- `Result::unwrap` and `Result::expect` append the error's `Debug` text. An error type on an
  FFI path must not carry input data in `Debug`, or the call must map the error before it
  panics.
- The JNI `ThrowRuntimeExAndDefault` policy copies the error's `Display` text into the
  exception. Use a fixed-message policy when that text can carry input data.
- Swift calls a non-throwing UniFFI export through `try!`, so a panic there ends the app. Keep
  such exports panic-free.
- `core::hint::unreachable_unchecked()` with a wrong proof is undefined behaviour, not a panic.
  Use it only with a `SAFETY` comment that proves the branch is impossible.

## Rule 1: decide what a panic does at each foreign entry point

Since Rust 1.81, a panic that reaches an `extern "C"` or `extern "system"` function aborts the
process with `panic in a function that cannot unwind`: the host sees a native crash. Before 1.81
it was undefined behaviour, so a crate with an MSRV below 1.81 must guard every entry point.
This skill is the catalog's single home for this policy:

- Guard with `catch_unwind` every entry point whose caller must get an error instead of a dead
  process: the default for a JNI library, a plugin, or a `cdylib` behind Swift or C.
- Leave a body unguarded only where an abort is the intended result, for example a callback in a
  standalone binary. Write that intent in a comment on the function.
- Use `extern "C-unwind"` only when the other side can unwind through it, such as C++ built
  with exceptions. A Java, Swift, or `-fno-exceptions` C caller cannot.
- A foreign exception that unwinds into Rust through a function declared `extern "C"` is
  undefined behaviour. Catch a C++ exception on the C++ side, or declare the import
  `extern "C-unwind"`.

## Choose the panic strategy before you write the guard

`panic = "abort"` silently turns every `catch_unwind` guard into dead code. Set
`panic = "unwind"` in the workspace-root `[profile.release]` when any entry point relies on
`catch_unwind`; a member manifest's profile has no effect, and tests always build with unwind.
Read `references/panic-strategy.md` when you set or review the `panic` key or its overrides.

## What `catch_unwind` catches

`std::panic::catch_unwind` catches an unwinding panic that starts inside the closure, on the
same thread, after the panic hook runs. It does not catch a panic on another thread (join it),
a process abort (a panic in `Drop` during unwinding, an `unsafe precondition(s) violated` check,
an allocation failure), a stack overflow, or, in any reliable way, a foreign exception (catch it
on the foreign side).

Do not remove a guard for speed without a benchmark that shows its cost.

Read `references/unwind-state.md` when guarded code mutates shared state, holds a lock, or needs `AssertUnwindSafe`.

## The guard pattern: hand-rolled C ABI

Return a status code, never a `Result` and never a panic. Reserve one code for a caught
panic so the caller can tell a domain error from a bug.

Read `references/boundary-patterns.md` (section "The guard pattern: hand-rolled C ABI") for the
complete `extern "C"` function, `lib_render`, and its `discard_panic_payload` helper.


Keep the `extern` body to a guard plus a delegation call. Put the logic in a plain Rust
function that the tests can call directly.

The payload's destructor can panic, so drop it inside a second guard, and leak a second payload
with `ManuallyDrop` (a bounded leak on the double-panic path only). Do not inspect or format
either payload.

Read `references/boundary-patterns.md` also when the entry point returns a pointer or an opaque
handle, writes out-parameters, is a callback that a foreign runtime invokes, or the crate needs a
macro-generated `extern` layer.

## JNI, UniFFI, and the panic hook

Read `references/jni-boundary.md` for a JNI export or `JNI_OnLoad`, `references/panic-hook.md` for the panic hook, and the UniFFI section of `references/boundary-patterns.md` for a UniFFI export.

## `.unwrap()` and `.expect()` on FFI-reachable paths

The `rust-discipline` skill owns the general `unwrap` and `expect` rule. On a path that a
foreign caller reaches, a panic kills the host process unless a guard catches it.

Never `.unwrap()` or `.expect()` there on a `Result` or `Option` that input or the environment
can make `Err` or `None`: propagate with `?` and let the boundary map the error. Never write
`.expect("should never happen")`: state the invariant in the message, or model it in the type.

Deny `clippy::unwrap_used`, `clippy::panic`, `clippy::todo`, `clippy::unimplemented`, and
`clippy::panic_in_result_fn` in every crate on an FFI path. Set `clippy::expect_used` and
`clippy::indexing_slicing` to at least `warn`. Count the debt with the command in `references/unwrap-audit.md` ("Step 1"), never from memory,
and read that file when you roll these lints out.

Read [the rollback guide](references/drop-guard.md) for rollback with
`DropGuard` on Rust 1.100+.

## Errors at the boundary

Keep the panic exit distinct from every error exit: a panic is a bug, an error is an expected
failure, and a shared code makes the crash report useless. Read the
typed-errors section of `references/unwrap-audit.md` when you design or map the error type of a
crate on an FFI path.

## Shared state, tasks, and threads

Read the async section of `references/boundary-patterns.md` when a panic can start in a spawned task, a thread, or a future that `block_on` drives.

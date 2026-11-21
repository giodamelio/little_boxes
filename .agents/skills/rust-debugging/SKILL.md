---
description: Use when debugging a Rust crash, panic, hang, or unreadable native stack trace, on the host or inside an Android or iOS app behind JNI or UniFFI. Triggers on RUST_BACKTRACE, SIGSEGV, SIGABRT, tombstone, logcat crash output, addr2line, ndk-stack, atos, dSYM, rust-lldb, rust-gdb, lldb-server, _RNv mangled frames, JNI panic, UniFFI panic, and tokio-console for async stalls. Not for slow code or profiling; use `rust-performance`.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-debugging
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: b770653c1c44d02d6d9b1f0ba0c76fa23d359dd6
name: rust-debugging
---
# Rust Debugging (Host First, Then Android and iOS)

Reproduce on the host first. A CLI or a test that drives the same code gives you a debugger, a
backtrace, sanitizers, and a fast loop. Move to a device only after the host repro fails. On a
device, go from logs to the tombstone or crash report, then symbolicate, and attach LLDB last.

## Where to start

| Symptom | Start here |
|---|---|
| Panic message and a Rust backtrace are visible | Host. Write a test that calls the same function. |
| Crash only under one input file or payload | Host. Feed the input to a CLI or a unit test. |
| Crash only on device, no panic message | Android or iOS. Pull the tombstone or crash report (sections 3 and 4). |
| SIGSEGV or SIGBUS with no Rust frames | Native memory bug. Symbolicate, then run the host repro under ASan (`-Zsanitizer=address` on nightly), or the device build under HWASan or MTE (the `rust-sanitizers-miri` skill). |
| Kotlin or Swift gets an error, Rust logs nothing | FFI boundary. See section 2. |
| Process hangs | Deadlock or async stall. Backtrace every thread, then see section 5. |
| Snapshot or golden output changed, no panic | Not a crash. Read the diff. Re-bless only in a commit that explains why (`cargo insta review` for `insta`; the `rust-test-tools` skill). |

## Panic and signal triage

| Signal or message | Likely cause | Next step |
|---|---|---|
| ``called `Option::unwrap()` on a `None` value`` | Unwrap on `None` | Find the optional field. Replace with `ok_or` plus `?`. |
| ``called `Result::unwrap()` on an `Err` value`` | Unwrap on an error | Propagate with `?` and keep the source error. |
| `index out of bounds: the len is N but the index is M` | Slice or `Vec` out of range | Check the index math against a length that came from input. |
| `attempt to subtract with overflow` | Integer underflow with overflow checks on | Use `checked_sub` or `saturating_sub`. A release build wraps silently, so it is a bug either way. |
| `attempt to multiply with overflow` | Overflow in size math | Use `checked_mul` before you allocate. |
| `panic in a function that cannot unwind` | A panic reached an `extern "C"` or `extern "system"` export with no guard | Guard the export (section 2). |
| Signal 6 (SIGABRT) | Double panic, `abort()`, `panic = "abort"` (the `Abort message` holds the panic text), or an unguarded export (no `Abort message`; a `panic_cannot_unwind` frame after symbolication) | Read the tombstone `Abort message` when it is present. Check the export guards, then reproduce on the host. |
| Signal 11 (SIGSEGV) | Null or dangling pointer, use-after-free in `unsafe` or in a C dependency | Symbolicate, then run the sanitizers of the SIGSEGV row in the table above. |
| Signal 7 (SIGBUS) | Misaligned or invalid memory access, often a bad pointer cast | Audit the `unsafe` cast (the `rust-unsafe` skill). |
| Process killed while it writes to a pipe or socket | SIGPIPE | On an iOS, Swift, or C host, ignore SIGPIPE during init (section 2). |
| Kotlin `InternalException`, or a Swift error outside your error enum | Rust panic caught by the UniFFI guard | Fix the panic. Do not add a catch-all variant. |
| Swift fatal error on a non-throwing UniFFI call | Panic in a non-throwing export | Make the export panic-free, or return `Result`. |
| `JNI DETECTED ERROR IN APPLICATION` in logcat | Wrong JNI usage: stale local ref, wrong signature, missing exception check | Call `env.exception_check()` after each JNI call that can throw, and keep each local ref inside its call (the `rust-jni` skill). |
| Frames print as `_RNv...` or `__RNv...` | The tool predates v0 mangling, the default since 1.97 | Upgrade the tool (binutils 2.36+, Linux perf 6.16+, newer than the perf of Ubuntu 24.04), or pipe through `rustfilt`. On macOS, use `c++filt -_`. |
| A grep for `_ZN` finds nothing | v0 symbols start with `_R` (`__R` in Mach-O) | Grep `_R`. `#[no_mangle]` exports keep their literal names. |
| Deadlock or hang, no CPU load | Lock ordering, or a channel with no sender | Attach the debugger and backtrace every thread. |

## Before you say "cannot reproduce"

- [ ] You tried a host repro with `RUST_BACKTRACE=1`, and under a debugger.
- [ ] The build under test has debug info, and it is not stripped.
- [ ] You symbolicated against the exact binary that crashed: same Build ID or UUID, not a rebuild.
- [ ] Logging starts before the suspect code path runs, and one test `log::info!` reaches the sink.
- [ ] You read the tombstone or crash report signal and `Abort message`, not only the app-level
      message.
- [ ] You ran the host repro under a sanitizer when the signal was 11 or 7.

## 1. Host debugging

### Backtraces

```bash
RUST_BACKTRACE=1 cargo run --locked -p my-cli -- <args>        # =full: every frame
RUST_BACKTRACE=1 cargo test --locked -p my-crate <test_name> -- --no-capture
RUST_BACKTRACE=1 RUST_LIB_BACKTRACE=0 cargo run --locked -p my-cli -- <args>  # no error-value captures
```

`--nocapture` is deprecated since 1.88; use `--no-capture`.
Read [references/backtraces-and-logging.md](references/backtraces-and-logging.md) when you diff
or grep backtraces, or when `anyhow` errors pay for a backtrace capture.

### Debugger

`rust-lldb` and `rust-gdb` are toolchain scripts that load the Rust formatters, so `String`,
`Vec`, `Option`, and `HashMap` print as Rust values. Run `rust-lldb target/debug/my-cli -- <args>`
(macOS, or Linux with LLDB) or `rust-gdb --args target/debug/my-cli <args>`. Break on
`__rustc::rust_panic` (LLDB also takes `b rust_panic`). On a hang, backtrace every thread first:
`thread backtrace all` in LLDB, `thread apply all bt` in GDB.

Read [references/rust-gdb-pretty-printers.md](references/rust-gdb-pretty-printers.md) when a
breakpoint does not resolve, a value prints as raw fields, you load formatters by hand (Xcode,
Android Studio, plain `gdb`, an IDE on Rust 1.98 and later), configure CodeLLDB, set a closure,
trait-method, or conditional breakpoint, or demangle symbols.

### Debug info

A debugger needs variable-level debug info. Symbolication needs line tables and the exact
unstripped binary. Ship the stripped artifact, archive the unstripped one, and symbolicate against
the archived copy. Read [references/debug-info.md](references/debug-info.md) when the debugger
shows no variables, a release backtrace has no file or line, or you archive an ELF, macOS, or
Apple build.

### Logging

`dbg!`, `println!`, and the default panic hook write to stdout or stderr. An Android app process
sends both to `/dev/null`, so none of them reach logcat. Use `tracing` for output that must
survive on a device. Enable `clippy::dbg_macro` (restriction group, allow by default) to catch a
leftover `dbg!`.

For a host repro, install a `tracing_subscriber::fmt()` subscriber with `FmtSpan::CLOSE` span
timing; [references/backtraces-and-logging.md](references/backtraces-and-logging.md) has the
setup and how to read `time.busy` and `time.idle`. The `rust-observability` skill owns the field
rules and the device subscriber setup.

## 2. Panics at the FFI boundary

The `rust-panic-safety` skill owns the guard and hook policy, and the `rust-jni` skill owns JNI
export patterns; without them, wrap each export body in `std::panic::catch_unwind`.

- Since Rust 1.81, a panic that reaches an `extern "C"` or `extern "system"` function aborts the
  process with SIGABRT. The symbolicated backtrace has `core::panicking::panic_cannot_unwind`
  above the export; on Android the panic text is lost. Read
  [references/ffi-boundary.md](references/ffi-boundary.md) for the host and tombstone output.
- Fix it at the export, not at the caller. On `jni` 0.22 use
  `EnvUnowned::with_env(...).resolve::<Policy>()`.

### UniFFI panic and error propagation

UniFFI generates the boundary guard. Read [references/ffi-boundary.md](references/ffi-boundary.md)
for what Kotlin and Swift see for `Ok`, `Err`, and a panic in a throwing or a non-throwing export.

- An exception that is not one of your declared variants is a Rust panic, not a handled error.
- Do not add an `Internal` variant and expect panics to arrive in it. They do not. Convert the
  failure into a real `Err` on the Rust side when the caller must handle it. Keep a non-throwing
  export panic-free: return `Result<T, E>` with a `#[derive(uniffi::Error)]` enum (the
  `uniffi-boundary` and `ffi-error-progress-cancel` skills own the mapping).

### Panic reports on a device

In a shipped build, report a panic only through the bootstrap's redacted hook (the
`rust-panic-safety` skill; without it, print only `info.location()` from
`std::panic::set_hook`). Never put the payload, a file path, or a backtrace into a shipped log,
because panic text can hold input data, paths, or secrets. On Android, `panic = "abort"` copies the panic message into the tombstone
`Abort message` line, so treat every panic message as a log line.

### SIGPIPE

A write to a pipe or a socket whose peer closed can raise SIGPIPE. Its default action kills the
process with no Rust panic and no log, and a host repro hides it. Read
[references/ffi-boundary.md](references/ffi-boundary.md) when a `cdylib` or `staticlib` dies on a
write in an iOS, Swift, C, or JVM host.

## 3. Android debugging

Go to the device only after the host attempt fails. Wire a logcat sink first, then read the
record and the signal (`adb logcat -d -b crash`), then symbolicate the tombstone with `ndk-stack`
or `llvm-addr2line` against the unstripped `.so` of the same build. Attach LLDB only when the log
and the tombstone both fall short.

Read [references/android-debugging.md](references/android-debugging.md) when you filter logcat,
route tracing to logcat, need `RUST_BACKTRACE` on a device, read a tombstone, run `addr2line` or
`ndk-stack`, or attach LLDB from Android Studio.

## 4. iOS debugging

Use the same order: the crash report, then symbolication, then LLDB in Xcode. Symbolicate
against the `.dSYM` of the app build, because the linker moves the `staticlib` code into the app
binary; its `dwarfdump --uuid` must match the image UUID in the crash report. The Xcode ASan and
TSan switches do not instrument a prebuilt Rust `staticlib`: run `-Zsanitizer=address` or
`-Zsanitizer=thread` on a host target (the `rust-sanitizers-miri` skill).

Read [references/ios-debugging.md](references/ios-debugging.md) when you symbolicate with `atos`,
check that the `.dSYM` holds Rust lines, build for a device or the simulator, or break in Rust
from Xcode.

## 5. Async debugging

Start with the `FmtSpan::CLOSE` span timing from section 1.

`tokio-console` shows live task state. It opens a local TCP port: use it in a local debug build
only, and never ship it in a mobile or production build. Read
[references/tokio-console.md](references/tokio-console.md) before you add it, and when you read
its warnings: it has the setup and the warning triage table (lost waker, never yielded,
self-wake, idle task).

On a device or a production build, read `tokio::runtime::RuntimeMetrics` instead ([references/tokio-console.md](references/tokio-console.md) lists the fields). The `rust-observability` skill covers how to export them.


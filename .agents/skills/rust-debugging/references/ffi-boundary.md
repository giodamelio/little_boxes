# FFI boundary: UniFFI panics and SIGPIPE

Deep material for `rust-debugging`, section 2.

## A panic at an `extern` export

Since Rust 1.81, a panic that reaches an `extern "C"` or `extern "system"` function aborts the
process. This is defined behavior. On the host, stderr shows the panic, then `panic in a
function that cannot unwind` and `thread caused non-unwinding panic. aborting.`, and the signal is
SIGABRT. On Android, stderr goes nowhere and the panic text is lost. The tombstone shows SIGABRT,
and the symbolicated backtrace has `core::panicking::panic_cannot_unwind` above the export.

## What the foreign side sees from UniFFI

| Rust value | Kotlin | Swift |
|---|---|---|
| `Ok(v)` | the return value | the return value |
| `Err(E)`, where `E` is an exported error enum | an exception subclass, one per variant | `enum E: Error`, one case per variant |
| a panic in a throwing export | `InternalException` with the panic message, **not** a subclass of your error type | a private UniFFI error, **not** a case of your error enum |
| a panic in a non-throwing export | `InternalException` | a fatal Swift error that no `catch` can handle; the app crashes |

- The panic message rides in the Kotlin exception text, in the Swift error description, and in
  the `try!` fatal-error message of a non-throwing Swift call. A crash reporter that records
  one of them records the message.

## SIGPIPE

A write to a pipe or a socket whose peer closed can raise SIGPIPE. Its default action kills the
process with no Rust panic and no log. A host repro hides this. The Rust runtime of a binary or a
test sets SIGPIPE to ignored before `main`, so the same write returns an error there:

```rust,run
use std::io::{ErrorKind, Write};

fn main() {
    let (reader, mut writer) = std::io::pipe().unwrap();
    drop(reader);
    // A Rust `main` starts with SIGPIPE ignored, so this is an error, not a kill.
    let err = writer.write_all(b"x").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::BrokenPipe);
}
```

A `cdylib` or `staticlib` gets no such setup. Inside a JVM it needs none: ART blocks SIGPIPE
and HotSpot installs its own handler, so the write returns `BrokenPipe`. Never change a signal
disposition there, because the runtime chains signals and owns the table. In an iOS, Swift, or
plain C host, let the application bootstrap ignore SIGPIPE during init, then handle
`ErrorKind::BrokenPipe` like any other error:

```rust
pub fn ignore_sigpipe() {
    // SAFETY: `signal` with `SIG_IGN` installs no handler code. Call it from init only.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
}
```

# Runtime Setup and Foreign Threads

Reference material for `SKILL.md`. Read it when you choose a mechanism for blocking work, write
the shared-runtime initializer, pick a runtime flavor, set a thread stack size, or make a thread
that tokio does not own start or wait for async work.

## Contents

- [Blocking work](#blocking-work)
- [Shared runtime](#shared-runtime)
- [Runtime flavors](#runtime-flavors)
- [Thread stacks](#thread-stacks)
- [Enter the runtime from a foreign thread](#enter-the-runtime-from-a-foreign-thread)
- [Rules for both shapes](#rules-for-both-shapes)
- [Adopt a received socket](#adopt-a-received-socket)
- [Tokio advisories](#tokio-advisories)

## Blocking work

| Work shape | Mechanism | Reason |
|---|---|---|
| Bounded CPU work, target < 100 ms | `spawn_blocking` | it returns the pool thread quickly |
| Occasional blocking syscall, short file I/O | `spawn_blocking` | same |
| Many CPU-bound jobs at once | a `Semaphore` around `spawn_blocking`, or `rayon` | the blocking pool admits up to 512 threads by default |
| Data-parallel compute (decode, geometry, raster) | `rayon` inside one `spawn_blocking`, or `rayon::spawn` plus a `oneshot` for the result | a `rayon` call blocks its caller until the parallel work ends |
| Indefinite blocking loop, watcher, persistent synchronous connection | `std::thread::spawn` | it would occupy a pool thread for ever |
| Anything on a `current_thread` runtime | `spawn_blocking` | `block_in_place` panics there |
| Blocking I/O in a synchronous engine with no runtime | keep it synchronous | a runtime adds cost and no benefit there |

Load data before a `rayon` region; do not do blocking I/O inside a `rayon` task.

A synchronous protocol client that runs on its own `std::thread`, outside the runtime, is a valid
design. Write a comment at the spawn site that states the intent, so that a later reader does
not "fix" it into async.

## Shared runtime

Build one runtime explicitly and share it. Each build creates a new thread pool, so do not build
a runtime per session, per request, or per FFI call. Set `worker_threads` explicitly on mobile
and embedded targets; the default is one thread per core.

```rust
use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> std::io::Result<&'static Runtime> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = Builder::new_multi_thread()
        .worker_threads(2) // constrain for the battery and CPU budget
        .thread_name("app-tokio")
        .enable_all()
        .build()?;
    // A racing caller may win; its runtime is kept and `rt` is dropped here.
    // Call this only outside the runtime (a foreign or main thread): the drop
    // of the losing runtime panics inside async code.
    Ok(RUNTIME.get_or_init(|| rt))
}
```

Name the threads. A named thread makes a stack dump readable.

## Runtime flavors

| Flavor | Use it for | Constraint |
|---|---|---|
| `multi_thread` | production I/O concurrency | `tokio::spawn` needs `Send + 'static` |
| `current_thread` | tests, and synchronous wrapper APIs that call `block_on` | `block_in_place` panics; `tokio::spawn` still needs `Send` |
| `LocalRuntime` (tokio 1.51+) | `!Send` tasks through `spawn_local`, with no `LocalSet` | cannot move between threads; build with `Builder::new_current_thread().enable_all().build_local(LocalOptions::default())` |

Find a large future with `clippy::large_futures` (pedantic), and `Box::pin` it. A stack overflow
in a library that a host process loads can show as a plain SIGSEGV in the host crash report.

## Thread stacks

`thread_stack_size` sets the stack of every thread the runtime spawns: the workers and the
`spawn_blocking` pool. It does not cover the thread that calls `block_on`, because
`Runtime::block_on` polls the root future on that thread. With no value set, the std default
applies (2 MiB, or `RUST_MIN_STACK`), so a smaller value removes headroom.

## Enter the runtime from a foreign thread

A foreign thread (a JNI call, a C callback, a platform service thread) cannot `.await`. It
enters the runtime through `block_on`. Two shapes exist.

**Shape A: hand off to a worker thread.** The foreign call returns at once. Use this when the
work is long-running and the caller must not block.

```rust
let worker = std::thread::Builder::new()
    .name("app-worker".into())
    // `block_on` polls the root future on this thread; size the stack for it.
    .stack_size(4 * 1024 * 1024)
    .spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.block_on(run_session(config, fd, cancel, stats))
        }));
        // Record Ok / Err / panic into shared state before the thread exits.
    });
// The foreign thread returns immediately.
// A later stop call cancels the work through the CancellationToken.
```

**Shape B: block the calling thread.** The foreign thread owns the work for its whole lifetime.
Use this only when the caller is a dedicated service thread that has nothing else to do. On a
`multi_thread` runtime, run the work on a worker and block only on its handle:
`rt.block_on(rt.spawn(fut))`. The foreign stack then holds only the `JoinHandle`, and a panic in
`fut` returns as a `JoinError`. `fut` must be `Send + 'static`.

## Rules for both shapes

1. Do not call `block_on` for long-running work on a callback thread that the host expects to
   return promptly.
2. Wrap the `block_on` body in `catch_unwind`. In Shape A, it records a panic into shared state
   before the thread ends. In Shape B, it turns an abort into an error for the host, because
   since Rust 1.81 a panic that reaches an `extern "C"` boundary aborts the process. Both need
   `panic = "unwind"`. The `rust-panic-safety` skill, when it is installed, owns that policy;
   confirm the shipped build with
   `cargo rustc --locked -p <boundary-crate> --lib --release --target <shipping-triple> -- --print cfg`.
3. Take ownership of a received file descriptor with a duplicate, because the host can close the
   original at any time. An `OwnedFd` closes on every error path with no cleanup code.
4. Set a received socket to nonblocking before `from_std` or `AsyncFd::register`. On Unix,
   `from_std` panics in debug builds on a blocking socket. `AsyncFd::register` does not check,
   and a blocking fd then blocks a worker thread inside a read in every build. Since tokio
   1.53.3 (backported to 1.51.6, per the tokio 1.54.1 CHANGELOG), use the unsafe `register` API instead of the deprecated `new`. Prove that the owned
   descriptor stays valid and open, refers to the same underlying file description, and that
   every `as_raw_fd()` call returns that same descriptor for the entire `AsyncFd` lifetime.
   Do not close or replace it through another owner or through `get_mut`. Call `register`
   inside a Tokio runtime with I/O enabled, and document these invariants at the unsafe call.
5. Reset shared state with an RAII guard, not with cleanup code at each early return. A guard that
   sets the module back to `Idle` on drop keeps the state correct after a panic too.
6. Store the `CancellationToken` in the state-machine variants that own live work (for example
   `Starting` and `Running`), so that stop and destroy paths can always reach it.

## Adopt a received socket

```rust
use std::os::fd::{BorrowedFd, OwnedFd, RawFd};

/// # Safety
/// `raw` must be an open socket for the duration of this call.
unsafe fn adopt_socket(raw: RawFd) -> std::io::Result<tokio::net::TcpStream> {
    // SAFETY: the caller keeps `raw` open for the duration of this call.
    let owned: OwnedFd = unsafe { BorrowedFd::borrow_raw(raw) }.try_clone_to_owned()?;
    let stream = std::net::TcpStream::from(owned);
    stream.set_nonblocking(true)?;
    // `from_std` panics outside a runtime context with I/O enabled.
    tokio::net::TcpStream::from_std(stream)
}
```

## Tokio advisories

Plain `cargo audit` exits 0 on an `unsound` advisory, and the cargo-deny default
`unsound = "workspace"` misses a transitive tokio. Show the resolved version with
`cargo tree --locked -i tokio`. Do not keep a hand-written minimum-version table: one floor
misses backport lines. For example, the unsound advisory RUSTSEC-2025-0023 (the broadcast channel
calls `clone` on a `Send + !Sync` value from several threads) is patched in `>=1.38.2,<1.39`,
`>=1.42.1,<1.43`, `>=1.43.1,<1.44`, and `>=1.44.2`. A floor of 1.42.1 accepts the affected
1.43.0, 1.44.0, and 1.44.1. Set `unsound = "all"` under `[advisories]` in `deny.toml`; the
`rust-security` skill, when it is installed, owns that policy.

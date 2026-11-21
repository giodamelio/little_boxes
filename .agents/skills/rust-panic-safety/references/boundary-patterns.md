# Boundary guard patterns

Every pattern here answers the same question: where does the panic stop, and what does the
foreign caller see instead. Pick the shape that matches the boundary, then keep it identical
across all entry points of the crate.

Contents:

- The guard pattern: hand-rolled C ABI
- Pattern selection
- Status codes
- Returning a pointer or an opaque handle
- Out-parameters
- Transferring a bounded panic category
- Callbacks that foreign code invokes
- The macro-generated `extern` layer
- UniFFI
- Async work behind a synchronous boundary
- Testing the guard

## The guard pattern: hand-rolled C ABI

`SKILL.md` gives the rules; this is the reference shape. It returns a status code and reserves
one code for a caught panic:

```rust
pub const OK: i32 = 0;
pub const ERR_INVALID_INPUT: i32 = -1;
pub const ERR_IO: i32 = -2;
pub const ERR_PANIC: i32 = -99;

fn discard_panic_payload(payload: Box<dyn std::any::Any + Send>) {
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)));
    if let Err(payload) = second {
        // The destructor of the first payload panicked. Do not drop the second payload here.
        let _leaked = std::mem::ManuallyDrop::new(payload);
    }
}

/// # Safety
/// `ptr` must be non-null, aligned, writable for `len` bytes, and unaliased for
/// this call. `len` must not exceed `isize::MAX`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lib_render(ptr: *mut u8, len: usize) -> i32 {
    if ptr.is_null() || len > isize::MAX as usize {
        return ERR_INVALID_INPUT;
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees the contract above.
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        render_into(buf)
    });
    match result {
        Ok(Ok(())) => OK,
        Ok(Err(Error::InvalidInput)) => ERR_INVALID_INPUT,
        Ok(Err(Error::Io(_))) => ERR_IO,
        Err(payload) => {
            discard_panic_payload(payload);
            ERR_PANIC
        }
    }
}
```

Every `Err(payload)` branch below calls this `discard_panic_payload` helper. This probe proves that it survives a payload whose destructor panics:

```rust,run
fn discard_panic_payload(payload: Box<dyn std::any::Any + Send>) {
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)));
    if let Err(payload) = second {
        let _leaked = std::mem::ManuallyDrop::new(payload);
    }
}

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("panic payload destructor");
    }
}

fn main() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        std::panic::panic_any(PanicOnDrop);
    });
    let payload = match result {
        Err(payload) => payload,
        Ok(()) => unreachable!(),
    };
    discard_panic_payload(payload);
    std::panic::set_hook(hook);
}
```

Do not format or inspect the payload. `ManuallyDrop` is a bounded leak on the
double-panic path, where safe destruction is no longer available.

## Pattern selection

| Boundary shape | Guard | Panic result the caller sees |
|---|---|---|
| C ABI, returns a scalar | `catch_unwind` in the `extern` body | A reserved negative status code |
| C ABI, returns a pointer or handle | `catch_unwind` in the `extern` body | Null, plus a last-error slot |
| C ABI, writes through out-params | `catch_unwind` in the `extern` body | A status code; out-params untouched |
| JNI method | `with_env` + `resolve`, or `catch_unwind` (see `jni-boundary.md`) | A thrown exception and a neutral return value |
| UniFFI `#[uniffi::export]` | Generated scaffolding | Kotlin: `InternalException` with the panic text. Swift: an internal error from a throwing export; an uncatchable fatal error from a non-throwing export |
| Rust callback that foreign code invokes | `catch_unwind` in the callback body | A status code, or a recorded flag |
| Runtime entry that drives async work | `catch_unwind(AssertUnwindSafe(...))` around `block_on` | A status code plus a join-point check |

## Status codes

Reserve a code for a caught panic. Never reuse a domain error code for a bug.

```rust
#[repr(i32)]
pub enum Status {
    Ok = 0,
    InvalidInput = -1,
    NotFound = -2,
    Io = -3,
    Busy = -4,
    Panic = -99,
}
```

Rules:

- Keep the numbers stable. A binding layer hard-codes them.
- Map the typed error to the code in one function, in one module.
- Require the outermost Rust FFI bootstrap's panic hook to include the
  component's privacy-safe handler before you return `Panic`. It records a
  closed site code plus numeric location. Never log or transfer the raw payload.

## Returning a pointer or an opaque handle

A constructor cannot return a status code and a pointer at the same time. Return null on
failure and expose the reason through a separate call.

```rust
#[derive(Clone, Copy)]
#[repr(i32)]
enum LastError {
    None = 0,
    Domain = 1,
    Panic = 2,
}

thread_local! {
    static LAST_ERROR: std::cell::Cell<LastError> =
        const { std::cell::Cell::new(LastError::None) };
}

fn set_last_error(error: LastError) {
    LAST_ERROR.with(|slot| slot.set(error));
}

#[unsafe(no_mangle)]
pub extern "C" fn lib_last_error_code() -> i32 {
    LAST_ERROR.with(|slot| slot.get() as i32)
}

/// # Safety
/// `config` must be non-null and point to a valid null-terminated C string for
/// the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lib_session_new(config: *const std::os::raw::c_char) -> *mut Session {
    if config.is_null() {
        set_last_error(LastError::Domain);
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: the contract above requires a valid C string, and the null
        // check rejects the only pointer condition that this function can test.
        let config = unsafe { std::ffi::CStr::from_ptr(config) }
            .to_str()
            .map_err(|_| Error::InvalidInput)?;
        Session::new(config)
    });
    match result {
        Ok(Ok(session)) => {
            // Reset the slot, so a stale code from an earlier call is not read as this one.
            set_last_error(LastError::None);
            Box::into_raw(Box::new(session))
        }
        Ok(Err(_)) => {
            set_last_error(LastError::Domain);
            std::ptr::null_mut()
        }
        Err(payload) => {
            discard_panic_payload(payload);
            set_last_error(LastError::Panic);
            std::ptr::null_mut()
        }
    }
}
```

The matching destructor must also be guarded, because a `Drop` implementation deeper in the
tree can panic.

```rust
/// # Safety
/// `ptr` must come from `lib_session_new` and must not be used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lib_session_free(ptr: *mut Session) {
    if ptr.is_null() {
        return;
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees single ownership of a pointer from the constructor.
        drop(unsafe { Box::from_raw(ptr) });
    });
    if let Err(payload) = result {
        discard_panic_payload(payload);
    }
}
```

A destructor that panics twice — once in the value, once in the guard — still aborts. Keep
`Drop` implementations infallible; the guard is a backstop, not a licence.

## Out-parameters

Write the out-param only on the success path. A caller that sees a non-zero status must be
able to trust that its buffer is unchanged.

```rust
/// # Safety
/// `input` must be non-null and valid for reads of `len` bytes, including when
/// `len` is zero, and `len` must not exceed `isize::MAX`. `out_len` must be
/// non-null and writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lib_measure(input: *const u8, len: usize, out_len: *mut usize) -> i32 {
    if input.is_null() || out_len.is_null() || len > isize::MAX as usize {
        return Status::InvalidInput as i32;
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees `input` is valid for `len` bytes.
        let bytes = unsafe { std::slice::from_raw_parts(input, len) };
        measure(bytes)
    });
    match result {
        Ok(Ok(value)) => {
            // SAFETY: the caller guarantees `out_len` is writable.
            unsafe { out_len.write(value) };
            Status::Ok as i32
        }
        Ok(Err(_)) => Status::InvalidInput as i32,
        Err(payload) => {
            discard_panic_payload(payload);
            Status::Panic as i32
        }
    }
}
```

## Transferring a bounded panic category

Return a stable integer category such as `LastError::Panic`. Do not transfer the
panic payload as a string. Truncation does not make arbitrary text safe. The
privacy-safe hook records a closed site code and numeric location for diagnosis.
Use a local host repro or an offline crash artifact when you need a backtrace or
message.

## Callbacks that foreign code invokes

A Rust function pointer handed to a C or platform runtime is an FFI entry point. It needs the
same guard, and it usually cannot report an error, so it must record one.

```rust
/// # Safety
/// `ctx` must be the live `State` pointer registered with this callback, and
/// the owner must prevent concurrent mutation for the duration of the call.
unsafe extern "C" fn on_event(ctx: *mut std::ffi::c_void, code: i32) -> i32 {
    if ctx.is_null() {
        return Status::InvalidInput as i32;
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: `ctx` is the pointer passed to the registration call and outlives it.
        let state = unsafe { &*(ctx as *const State) };
        state.handle(code)
    });
    match result {
        Ok(Ok(())) => Status::Ok as i32,
        Ok(Err(_)) => Status::InvalidInput as i32,
        Err(payload) => {
            discard_panic_payload(payload);
            // Latch the stable category so the next owning call can report it.
            // Relaxed: the code is the whole message, and no other write is published.
            CALLBACK_FAILURE.store(
                Status::Panic as i32,
                std::sync::atomic::Ordering::Relaxed,
            );
            Status::Panic as i32
        }
    }
}
```

Rules:

- A callback that a foreign event loop calls must never unwind, even when the loop claims to
  tolerate errors.
- Do not attach the panic to the current call only. Latch it, and surface it at the next call
  the owner makes, so the failure is not lost.
- Latch only a stable category or status code. Discard the arbitrary panic payload.
- Keep the callback body short. Long work inside a foreign callback multiplies the ways a
  panic can start.

## The macro-generated `extern` layer

When a crate exports many entry points, generate the `extern` layer so no hand-written body
can drift from the pattern. A reviewer then checks one macro instead of forty functions.

```rust
macro_rules! ffi_entry {
    ($name:ident, ($($arg:ident: $arg_ty:ty),* $(,)?), $ret:ty, $entry:ident, $on_panic:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name($($arg: $arg_ty),*) -> $ret {
            match std::panic::catch_unwind(move || $entry($($arg),*)) {
                Ok(value) => value,
                Err(payload) => {
                    discard_panic_payload(payload);
                    $on_panic
                },
            }
        }
    };
}
```

Pass a fixed status code, null pointer, or other documented sentinel as
`$on_panic`. The macro discards the payload. The process-wide privacy-safe hook
records the bounded site information once.

Audit that nothing bypasses it:

```bash
# Every extern definition should come from the macro. Hand-written bodies are findings.
rg -n 'extern ("(C|system)" )?fn' --type rust
```

## UniFFI

`uniffi::setup_scaffolding!()` with `#[uniffi::export]` generates the `extern "C"` glue and
its panic guard. Consequences:

- Generated scaffolding needs no hand-written guard. Run
  `rg -n 'extern ("(C|system)"|fn)' <crate>/src`. Generated scaffolding does not appear there, so
  every hit is a hand-rolled export.
- Any hand-rolled `extern "C"` in the same crate bypasses the generated guard and needs the
  full treatment.
- Model the failure as a typed error in the exported signature: return `Result<T, E>` with
  `#[derive(uniffi::Error)]` on `E`. A typed error keeps its variant. A panic becomes an internal error on the foreign side, and its message is
  the panic payload text (`&str` or `String`). Keep panic messages free of input data, and do
  not forward that exception message to telemetry.
- Swift calls a non-throwing export through `try!`, so a panic there ends the app. Keep such
  exports, for example a cancel call, panic-free, or give them a typed error.

The `uniffi-boundary` skill owns the type mapping: every fallible export returns
`Result<_, E>`, and `E` derives `uniffi::Error`. The `ffi-error-progress-cancel` skill owns the
error, progress, and cancellation contract: map `Cancelled` to `CancellationException` in Kotlin
and `CancellationError` in Swift, never to a domain error.

## Async work behind a synchronous boundary

```rust
/// # Safety
/// `handle` must be a non-null, live, exclusively borrowed `Session` pointer
/// for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lib_run_blocking(handle: *mut Session) -> i32 {
    if handle.is_null() {
        return Status::InvalidInput as i32;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: the caller owns `handle` for the duration of this call.
        let session = unsafe { &mut *handle };
        session.runtime.block_on(session.run())
    }));
    let poison = || {
        // SAFETY: the entry-point contract keeps `handle` live for this call.
        // The independent atomic does not inspect potentially torn state.
        // Relaxed: the flag publishes no other write; a reader only fails fast.
        unsafe { &*handle }
            .poisoned
            .store(true, std::sync::atomic::Ordering::Relaxed);
    };
    match result {
        Ok(Ok(())) => Status::Ok as i32,
        Ok(Err(SessionError::TaskPanicked)) => {
            poison();
            Status::Panic as i32
        }
        Ok(Err(_)) => Status::InvalidInput as i32,
        Err(payload) => {
            poison();
            discard_panic_payload(payload);
            Status::Panic as i32
        }
    }
}
```

Two distinct failure paths exist here:

1. A panic in the future's own body unwinds through `block_on`, and `catch_unwind` sees it.
2. A panic in a spawned task does not. It surfaces as a `JoinError` at the join
   point. The `run()` body must check every handle it keeps, dispose of the
   payload through the guarded helper, and return `SessionError::TaskPanicked`
   so the entry point poisons the session.

The join-point check inside `run()`:

```rust
match handle.await {
    Ok(value) => Ok(value),
    Err(err) if err.is_panic() => {
        discard_panic_payload(err.into_panic());
        Err(SessionError::TaskPanicked)
    }
    Err(_) => Err(SessionError::Cancelled),
}
```

A `std::thread` handle works the same way: `join()` returns `Err(payload)`. Discard the payload
after you classify the result as a fatal internal error. A detached task whose `JoinHandle` you
drop reports nothing, so keep the handle or guard the task body itself.

`AssertUnwindSafe` does not restore a mutated `Session`. Give the session an
independent poison flag, check it at every entry point, and set it before you
return the panic status. A handle registry can instead invalidate the handle.
Do not let the caller reuse state whose invariants a panic may have torn.

Both must reach the caller. A guard alone is not enough when the crate spawns.

## Testing the guard

- Call the plain Rust `_entry` function from a test to check the domain results. It has no
  guard, so it cannot test the panic code.
- Call the guarded `extern` function itself from a Rust test, or a hidden test-only entry point
  that uses the same guard. Force a panic through a test hook, then assert the status is the
  reserved panic code and the process is still alive.
- Cargo builds tests with unwind whatever the profile says, so a green guard test does not
  prove that the shipped build unwinds. Run the `--print cfg` check in `SKILL.md` for that.
- Add a debug-build assertion that the Rust bootstrap's composed hook includes
  this component's handler before the first entry point does work.
- Fuzz the input decoder behind the boundary. A fuzz harness finds the panics that a guard
  would otherwise hide behind a status code. See the `rust-test-tools` skill.

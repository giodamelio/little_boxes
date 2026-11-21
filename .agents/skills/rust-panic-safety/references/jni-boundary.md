# JNI panic boundaries

Read this file when you write or review a JNI export or `JNI_OnLoad`. The `rust-jni` skill owns
the binding rules: symbol names, thread attachment, local references, and class loading. For
example, wrap a loop that creates local references in `with_local_frame`. This
file covers only where the panic stops and what Java sees.

Contents:

- Loader: `JNI_OnLoad`
- Method entry point on `jni` 0.22
- Error policy with a fixed message
- Method entry point on `jni` 0.21
- Rules that hold for every JNI export

Every `Err(payload)` branch calls the `discard_panic_payload` helper from `SKILL.md`. Only
`JNI_OnLoad` can leak the payload instead.

## Loader: `JNI_OnLoad`

The `rust-jni` skill, when it is installed, owns the full `JNI_OnLoad` template: the VM
parameter type, native method registration, and class loading. Without it, start from this
panic skeleton on `jni` 0.22 and put the registration in `register_natives`:

```rust
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::JavaVM;
use jni::sys::{JNI_ERR, JNI_VERSION_1_6, jint};

/// # Safety
/// Only the JVM calls this, with a valid `JavaVM` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(raw_vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    let init = catch_unwind(AssertUnwindSafe(|| -> jni::errors::Result<()> {
        // Only when this library is the application-owned bootstrap.
        install_bootstrap_panic_hook();
        // SAFETY: the JVM passes a valid JavaVM pointer that stays valid for the process.
        let vm = unsafe { JavaVM::from_raw(raw_vm) };
        register_natives(&vm)
    }));
    match init {
        Ok(Ok(())) => JNI_VERSION_1_6,
        Ok(Err(_)) => JNI_ERR,
        Err(payload) => {
            // Runs once, so a bounded leak is safe; see the rules below.
            let _leaked = std::mem::ManuallyDrop::new(payload);
            JNI_ERR
        }
    }
}
```

The panic rules of any `JNI_OnLoad`:

- Guard the whole body with a raw `catch_unwind`. `JNI_OnLoad` gets the VM, not an env, so
  `EnvUnowned::with_env` is not available.
- Call `JavaVM::from_raw` inside the guard. It asserts non-null, and an assert outside the guard
  aborts the load.
- Return `JNI_ERR` for a caught panic, so the load fails before any worker starts.
- Pass the payload to `discard_panic_payload`. Never drop it outside a guard. A leak is also
  bounded, because `JNI_OnLoad` runs once: write `ManuallyDrop::new(payload)`. Do not write
  `mem::forget`: it fails the `rust-lints` floor (`mem_forget = "deny"`).
- Install the panic hook inside the same guard, when this library is the application-owned
  bootstrap.
- A panic that fires before the bootstrap installs the panic hook is contained, but the custom
  handler cannot report it. Do not claim otherwise in a review.

## Method entry point on `jni` 0.22

`jni` 0.22 catches the panic inside `EnvUnowned::with_env` and returns a `#[must_use]`
`EnvOutcome`. You exit through `resolve`, which rebuilds an `Env` and applies an `ErrorPolicy`
to the error and to the caught panic. It is the only place an exception can be thrown.

Use the entry template in the `rust-jni` skill, when it is installed: the body is one
`env.with_env(..).resolve::<Policy>()` expression.

`ThrowRuntimeExAndDefault` throws `java.lang.RuntimeException` and returns the default value.
It throws nothing when an exception is already pending. Its message is `Rust error: {err}`
with the error's `Display` text, or `Rust panic: {msg}` when the payload is a constant
`&'static str`. Any other payload becomes `non-string panic payload`. `LogErrorAndDefault` and
`LogContextErrorAndDefault` write the same kind of text through the `log` crate. Use these
policies only when every error `Display` string is safe for a Java log or crash report.
Otherwise resolve with the fixed-message policy below.

- `into_outcome()` gives the raw `Outcome::{Ok, Err, Panic}` instead of a resolved value. Take
  it only when the exit does not throw: after that call there is no `Env` left to throw with.
- Read the `with_env` documentation for the `jni` version in your lock file. The helper names,
  and the point at which the exception is thrown, changed between versions.
- Generate the `extern` layer with a macro when the crate exports many methods, so no
  hand-written body can drift from the pattern.
- `native_method!` generates the same `with_env` guard. An entry marked `raw`, or one that sets
  `catch_unwind = false`, has no guard: treat it as an unguarded entry point.

## Error policy with a fixed message

`ThrowRuntimeExAndDefault` copies the error's `Display` text and a constant panic message into
the Java exception. Use this policy when that text can carry input data, paths, or identifiers.
It throws one fixed message per outcome and discards the payload.

```rust
use jni::Env;
use jni::errors::ErrorPolicy;

pub struct ThrowFixedAndDefault;

impl<T: Default, E: std::error::Error> ErrorPolicy<T, E> for ThrowFixedAndDefault {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _err: E,
    ) -> jni::errors::Result<T> {
        // A pending Java exception already describes the failure.
        if !env.exception_check() {
            let _ = env.throw("native operation failed");
        }
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        discard_panic_payload(payload);
        if !env.exception_check() {
            let _ = env.throw("native operation panicked");
        }
        Ok(T::default())
    }
}
```

Resolve with `.resolve::<ThrowFixedAndDefault>()`. `env.throw` with a string throws
`java.lang.RuntimeException`. The panic hook still records the bounded site information.

## Method entry point on `jni` 0.21

Use this when the workspace pins `jni` 0.21 or earlier, which has no `with_env`/`resolve`.

```rust
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_app_NativeBridge_nativeStart(
    mut env: JNIEnv<'_>,
    _thiz: JObject,
    handle: jlong,
) -> jint {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let session = SESSION_REGISTRY.lookup(handle)?;
        session.start()
    }));
    match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(SessionError::InvalidHandle)) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "invalid native session handle",
            );
            -1
        }
        Ok(Err(_)) => {
            let _ = env.throw_new("java/lang/RuntimeException", "native operation failed");
            -1
        }
        Err(payload) => {
            discard_panic_payload(payload);
            let _ = env.throw_new("java/lang/RuntimeException", "native operation panicked");
            -1
        }
    }
}
```

`SESSION_REGISTRY.lookup` treats the `jlong` as an opaque generational ID. It
decodes a non-zero slot and generation, locks the registry, checks both values,
and clones an `Arc<Session>` only after they match. It releases the lock before
`start`. `nativeDestroy` removes the entry and increments its generation before
the slot can be reused. A forged, zero, destroyed, or stale ID returns
`SessionError::InvalidHandle`; it is never cast to a pointer.

## Rules that hold for every JNI export

- Throw before you return. The JVM raises the pending exception when control returns to Java.
- Return a neutral value after a throw. Java never reads it.
- Never call another JNI function after `throw_new` except to return. Most JNI calls are
  invalid while an exception is pending.
- Do not invent a new Java exception class per panic class. One type keeps one catch site on
  the managed side.
- Keep the panic exception message fixed. Never copy the panic payload into it. The privacy-safe
  hook records only a closed site code plus bounded numeric location.
- Resolve every `jlong` session ID through a generational registry before use. Never cast a
  caller-provided integer to a pointer.
- A panic on a thread that Rust attached to the JVM must be caught on that thread. The
  attaching code is an entry point too.

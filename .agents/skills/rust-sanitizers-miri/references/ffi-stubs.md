# FFI Stubs for Miri

Extended reference for the `rust-sanitizers-miri` skill. Read this when a Miri run reaches a
foreign call, when you write a `#[cfg(miri)]` stub, or when foreign code keeps a pointer into a
`Box`. The worked `Box` stubs and their test are in `references/miri-ub-patterns.md`.

## What Miri can execute

Miri cannot execute a foreign function that has no Miri shim. This includes
every C or C++ entry point behind a `-sys` crate, JNI calls, generated UniFFI
scaffolding, and inline assembly. Miri shims a subset of the platform API,
including files (with `-Zmiri-disable-isolation`), threads, and `epoll` and
`eventfd` on Linux targets. System API support varies between targets. If a shim
is missing, try `--target x86_64-unknown-linux-gnu` before you stub.
`-Zmiri-native-lib` calls real native code, but it is experimental and unsound.
A run through it is not evidence for the FFI path.

## Pick a strategy

Run Miri over the Rust logic while the foreign call is replaced or skipped.
Select the strategy from the shape of the dependency.

| FFI situation | Strategy |
|---|---|
| `extern "C"` block that you declare in your own crate | Gate the block with `#[cfg(not(miri))]`. Add a `#[cfg(miri)]` stub with the same signature. |
| Inline `asm!` or `global_asm!` | Add a pure-Rust fallback behind `#[cfg(miri)]`. Keep both paths under one test. |
| Third-party crate that links C or C++ | You cannot stub it. Exclude the crate from the Miri run, or gate the tests that reach it. |
| Generated FFI scaffolding, for example a UniFFI or JNI binding layer | Do not stub the generated code. Gate the tests that cross the boundary. |
| Test that needs a live host runtime, for example a JVM, a GPU driver or a database | Gate the test with `#[cfg_attr(miri, ignore)]`. Do not stub. |
| Platform syscall through `libc` with a simple return value | Stub it behind `#[cfg(miri)]` and return the success value. |

## Stub a foreign function that you declare

```rust
#[cfg(not(miri))]
unsafe extern "C" {
    fn platform_specific_call(fd: i32) -> i32;
}

#[cfg(miri)]
unsafe fn platform_specific_call(_fd: i32) -> i32 {
    0 // Deterministic success value for the interpreter.
}
```

The stub must keep the signature, the safety contract and the return domain of
the real function:

- If the real function returns a pointer that the caller dereferences, the stub
  returns a pointer into a real allocation. A null stub moves the defect
  instead of removing it.
- If the real function stores a pointer, the stub stores it too, and the stub
  of each later call that uses it dereferences it. Otherwise Miri cannot see an
  alias against the stored pointer ("Ownership patterns for a `Box`" below).
- Do not let a stub hide the UB that you want to find. A stub that always
  returns `0` for a function whose error path frees a buffer removes the test
  you need.

Keep the stub next to the real declaration, in the same module, so it cannot
drift from the signature. Use the built-in `cfg(miri)`, not a `miri` Cargo
feature: Miri sets the cfg itself, and a feature can be enabled by accident in
a normal build.

## Skip a test or exclude a crate

Put a comment with the reason on each `#[cfg_attr(miri, ignore)]`, and name
the job that covers the path instead.

```bash
# Select the crates that Miri can interpret.
cargo +nightly miri test --locked -p my-core -p my-parser

# Or run the workspace and exclude the crates that reach foreign code.
cargo +nightly miri test --locked --workspace --exclude my-ffi
```

Every path that a stub, a skip or an exclusion removes from Miri needs an ASan
run on the host, or a HWASan or MTE run on a device. A `cargo +nightly careful
test` run without `-Zcareful-sanitizer` is not a substitute: it adds std debug
assertions, not memory-error detection.

## Ownership patterns for a `Box`

- Transfer ownership with `Box::into_raw` and never use the original `Box`
  again. Access the value only through the raw pointer. Recover it with
  `Box::from_raw` exactly once, after the foreign side unregisters it and no
  foreign call or callback can still use the pointer. An unregister call can
  return while a callback on a foreign thread still runs.
- For a synchronous foreign borrow, do not access or reborrow the value until
  the call returns.
- Use `Pin<Box<T>>` only when `T` has a pinning invariant. It keeps the address
  stable. It does not permit aliasing or concurrent foreign access.

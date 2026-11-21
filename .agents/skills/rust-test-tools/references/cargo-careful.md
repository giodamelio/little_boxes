# cargo-careful lane

Deep reference for the cargo-careful section of `SKILL.md`.

cargo-careful rebuilds the standard library with debug assertions and adds some run-time
checks. It runs code that Miri cannot run (foreign calls, system calls), and it is much faster
than Miri. It checks these items:

- `ptr.read()` and `ptr.write(v)` get an aligned, non-null pointer.
- The std collections keep their internal invariants.
- `mem::zeroed` and `mem::uninitialized` panic on a type that does not allow that value
  (`-Zstrict-init-checks`).
- Const evaluation gets extra UB checks (`-Zextra-const-ub-checks`).

It does not check aliasing, general reads of uninitialized memory, use-after-free, or data
races. The upstream README says: "there is a lot of Undefined Behavior that is *not* detected by
`cargo careful`".

## Setup and commands

```bash
# Setup. careful needs a nightly from the last three months and the rust-src component.
# Install rust-src up front. Outside CI, careful stops and asks on stdin before it installs it.
rustup toolchain install nightly --component rust-src
cargo install --locked cargo-careful

cargo +nightly careful test --locked -p <ffi-crate> --no-fail-fast
cargo +nightly careful nextest run --locked -p <ffi-crate>

# Experimental: the same run with AddressSanitizer.
cargo +nightly careful test --locked -p <ffi-crate> -Zcareful-sanitizer=address
```

careful reads `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, and `build.rustflags`. It does not read
`target.<triple>.rustflags`. Move a flag that the tests need into one of the others for this lane.

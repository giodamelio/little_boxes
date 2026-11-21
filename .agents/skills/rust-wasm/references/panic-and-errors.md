# Panic and error behavior

Use this file when you choose how a WebAssembly build reports failure, install
a panic hook, or decide whether an instance survives a trap.

## Handle panic and error behavior

The standard WebAssembly targets use abort-oriented panic behavior by default.
A panic becomes a trap. Do not use `catch_unwind` as the normal error path and
do not assume a host can continue the same instance after a trap. Unwinding
needs nightly `-Zbuild-std`, so do not design a stable product around it.

- Return `Result` for input, network, capability, and domain failures.
- Install `console_error_panic_hook` in debug browser and Node.js builds when a
  readable JavaScript console stack is useful. Its repository is archived; the
  crate still works, but expect no fixes.
- Keep panic messages free of secrets and untrusted full payloads.
- Recreate or discard an instance after a trap when state integrity matters.
- Test the release profile. Debug-only panic output is not release behavior.

A panic hook improves diagnostics. It does not recover the computation. Use the
`rust-panic-safety` skill, when it is installed, when a library changes the
panic strategy or depends on unwind behavior. Without it, confirm the strategy
with `rustc --print cfg --target <triple>`: under `panic="abort"`,
`catch_unwind` catches nothing.

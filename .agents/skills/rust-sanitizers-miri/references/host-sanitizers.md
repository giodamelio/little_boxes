# Host Sanitizer Build Rules

Extended reference for the `rust-sanitizers-miri` skill. `SKILL.md`, section 4, has the
commands for ASan. This file has the TSan and MSan commands, the build-failure triage, and the
rules behind them.

## Commands

```bash
# ThreadSanitizer (Linux, macOS)
RUSTFLAGS="-Zsanitizer=thread" RUSTDOCFLAGS="-Zsanitizer=thread" \
    cargo +nightly test --locked -Zbuild-std --target "$HOST"

# MemorySanitizer (Linux; every linked object must be instrumented)
RUSTFLAGS="-Zsanitizer=memory -Zsanitizer-memory-track-origins" \
RUSTDOCFLAGS="-Zsanitizer=memory -Zsanitizer-memory-track-origins" \
    cargo +nightly test --locked -Zbuild-std --target "$HOST"
```

## Build failures

| Symptom | Probable cause | Next action |
|---|---|---|
| ``mixing `-Zsanitizer` will cause an ABI mismatch in crate`` | A crate or std was built without the sanitizer | Add `-Zbuild-std`; for doctests, set `RUSTDOCFLAGS` to the `RUSTFLAGS` value |
| Doctests fail to link with undefined `__asan_*` symbols | `RUSTDOCFLAGS` lacks the `-Zsanitizer` flag | Set `RUSTDOCFLAGS` to the `RUSTFLAGS` value |
| rustc aborts while it expands a proc macro under a sanitizer | `--target` is missing, so the proc macro was instrumented | Pass `--target "$HOST"` |
| MSan reports uninitialized reads in a C or C++ dependency | That object was built without MSan | Rebuild it with clang `-fsanitize=memory`, or use Miri |

## Rules

Measured (build failures measured on nightly 2026-11-09,
`x86_64-unknown-linux-gnu`; ASan defaults on nightly 2026-05-15,
`aarch64-apple-darwin`):

- Pass `-Zbuild-std`. The prebuilt standard library is not instrumented.
  Without the rebuild, TSan and MSan stop with
  ``mixing `-Zsanitizer` will cause an ABI mismatch in crate``, and ASan builds
  but leaves std unchecked.
- Pass `--target`, also for the host. Then Cargo keeps `RUSTFLAGS` off build
  scripts and proc macros. An instrumented proc macro can abort rustc.
- Set `RUSTDOCFLAGS` to the same `-Zsanitizer` value, or every doctest fails
  to build (section 3).
- On macOS, set `ASAN_OPTIONS=detect_stack_use_after_return=1:detect_leaks=1`.
  Without it, ASan misses `stack-use-after-return` and does not check leaks.
  Linux enables both by default, so an intentional leak fails a Linux ASan
  run. Set `detect_leaks=0` only for that job.
- Build every C and C++ dependency with clang `-fsanitize=memory` for an MSan
  run. Otherwise do not trust an MSan report.
- Expect TSan false positives on `std::sync::atomic::fence` and on
  synchronization in inline assembly. TSan does not model either one.

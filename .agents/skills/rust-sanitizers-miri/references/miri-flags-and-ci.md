# Miri Flags and CI Jobs

The full MIRIFLAGS table and a GitHub Actions example for the jobs in
`SKILL.md`, and the flag hazards. The job commands are in `SKILL.md`, section 9.
The CI rules that protect secrets are in `SKILL.md`, section 5 ("Nightly and cache rules").

## Flag hazards

- A misaligned read passes default Miri on some seeds and fails on others. Add
  `-Zmiri-symbolic-alignment-check` to make the failure certain. It gives false
  positives when code aligns a pointer with manual integer arithmetic;
  `align_to` is fine in both modes.
- `-Zmiri-strict-provenance` rejects every deliberate exposure, including
  `with_exposed_provenance`. Run exposing code in a separate job without the
  flag. No `cfg` reports the flag, so `cfg_attr(miri, ignore)` cannot select
  these tests. Keep them out of the gate with `-- --skip <name>` or by crate
  (`--exclude`). `-Zmiri-permissive-provenance` only silences the warning.
  Miri can miss bugs on exposed pointers either way.
- Miri reports 1 CPU by default, so `cargo miri test` runs one test at a
  time and does not detect a race between two tests on a shared resource
  (measured on nightly 2026-11-09). Add `-Zmiri-num-cpus=4` or
  `-- --test-threads=4` for that job. `RUST_TEST_THREADS` does not reach the
  program under isolation. `cargo miri nextest run` runs each test in its own
  process and never detects such a race.

## MIRIFLAGS reference

| Flag | Effect | Use it for |
|---|---|---|
| `-Zmiri-disable-isolation` | Allow host environment variables, file systems, the real-time clock and randomness | Tests that read files, the environment, or `SystemTime::now` |
| `-Zmiri-strict-provenance` | Stop on any integer-to-pointer cast or `ptr::with_exposed_provenance` | Code that never exposes provenance on purpose |
| `-Zmiri-permissive-provenance` | Silence the integer-to-pointer warning | The separate job for code that exposes provenance on purpose |
| `-Zmiri-symbolic-alignment-check` | Judge alignment from the allocation and the offset, not the concrete address | Byte parsers and unaligned-pointer code |
| `-Zmiri-many-seeds[=A..B]` | Run once per seed, `0..64` by default; stop at the first failing seed | Concurrent code |
| `-Zmiri-many-seeds-keep-going` | Try every seed after a failure | Counting the failing schedules |
| `-Zmiri-seed=N` | Set the seed that resolves non-determinism (default 0) | Reproducing one failing seed |
| `-Zmiri-deterministic-concurrency` | Make scheduling fully deterministic | Stable output; it misses schedule-dependent bugs |
| `-Zmiri-num-cpus=N` | Report N CPUs to the program (default 1). libtest and any pool sized from `available_parallelism` then run up to N threads; the Miri scheduler does not change | Code that sizes a pool from `available_parallelism`; running tests concurrently |
| `-Zmiri-ignore-leaks` | Skip the leak check at exit | A job whose code leaks on purpose (`Box::leak`, `mem::forget`) or leaves threads running at exit |
| `-Zmiri-tree-borrows` | Use Tree Borrows instead of Stacked Borrows | The second aliasing job (`SKILL.md`, section 6) |

## CI jobs

```yaml
- name: Miri
  run: |
    rustup toolchain install nightly --component miri
    cargo +nightly miri setup
    # Select only the crates that contain no foreign code.
    cargo +nightly miri test --locked -p my-core -p my-parser
  env:
    MIRIFLAGS: "-Zmiri-strict-provenance"

- name: Miri (Tree Borrows)
  run: cargo +nightly miri test --locked -p my-core
  env:
    MIRIFLAGS: "-Zmiri-tree-borrows -Zmiri-strict-provenance"

# In a job with matrix.sanitizer: [address, thread]
- name: Sanitizer
  run: |
    rustup toolchain install nightly --component rust-src
    cargo +nightly test --locked -Zbuild-std --target x86_64-unknown-linux-gnu
  env:
    RUSTFLAGS: "-Zsanitizer=${{ matrix.sanitizer }}"
    RUSTDOCFLAGS: "-Zsanitizer=${{ matrix.sanitizer }}"
```

- Run the Miri jobs and the sanitizer jobs in parallel. They share no
  artifacts.
- Keep the sanitizer jobs on the host target, also for a crate that ships to a
  device. The host run is faster and executes the same parser and decoder
  paths. A cross-compiled sanitizer run needs a device or an emulator. Put it
  in a scheduled or on-demand job.

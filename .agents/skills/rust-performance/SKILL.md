---
description: Use when a Rust workload is slow, a benchmark regressed, a binary or app bundle grew, or a build is slow, and the next step is to measure with a flamegraph, samply, perf, simpleperf, Perfetto, Instruments, DHAT, Criterion, Gungraun, cargo-bloat, cargo-llvm-lines, or cargo --timings, or to tune LTO, codegen-units, opt-level, PGO, a rayon thread pool, sccache, or mold. Not for the code change once a profile names the hotspot; use `rust-hot-path`.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-performance
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: de118ad3dd2be848ac6dea68f201b3662ec5298e
name: rust-performance
---
# Rust Performance

Measure speed, heap use, binary size, and build time on the host, Android, and iOS. The `rust-hot-path` skill, when it is installed, turns a named hotspot into a code change; the `rust-observability` skill owns tracing spans and structured timing in production.

## Rules of engagement

1. Name one metric (items/s, MB/s, ms per pass, peak MB, bundle KB) and record a baseline before you change code. A change without a before and an after number is a guess.
2. Measure the build that ships; `dev` numbers do not transfer. On the host, profile the `profiling` profile below (release plus symbols). On a device, profile the ship profile with line tables and no strip.
3. Change one thing per measurement, on the same machine, power state, and device.
4. Fix the algorithm or the data structure before you tune build flags.

## Build a profile that the profiler can read

A profile without symbols shows hex addresses and truncated stacks. A release build has no debug info (Cargo passes `-C strip=debuginfo` whenever `debug` is off, as of Cargo 1.99.1). Define this profile once in the workspace-root `Cargo.toml`:

```toml
[profile.profiling]
inherits = "release"
debug = true      # or "line-tables-only" for file:line only
strip = "none"    # required: `strip` is inherited, so a stripping release strips this too
```

Build it with `cargo build --locked --profile profiling --bin myapp`, and point every profiler at `target/profiling/<name>`. Check the build before you read a profile: on Linux, `readelf -S target/profiling/myapp | grep -c debug_info` must print a non-zero count, and on every host the flamegraph must show function names.

## Tool selection

| Target | CPU profile | Heap profile | Notes |
|--------|-------------|--------------|-------|
| Host (Linux) | `samply`, `cargo flamegraph`, `perf record` | `heaptrack`, DHAT | `kernel.perf_event_paranoid` at 1 or lower |
| Host (macOS) | `samply`, `cargo flamegraph` (`xctrace`), Instruments | the `dhat` crate, Instruments Allocations | Do not weaken SIP |
| Android | `simpleperf`, Perfetto | Android Studio native allocations | No `perf`, `heaptrack`, or DHAT |
| iOS | Instruments Time Profiler, `os_signpost` | Instruments Allocations and Leaks | MetricKit for field data |

## Verification

Before you claim a performance change, have evidence for each line that applies.

- The hotspot: a profile (host `profiling`, device ship profile) or a benchmark named it before the change.
- The win: a Criterion `--save-baseline before` and `--baseline before` pair (section 6) with a change interval that does not cross zero, or a Gungraun instruction-count delta. A green `cargo bench --no-run` proves only that the benchmarks compile.
- The benchmark measured work: the two-size scaling check in section 6 moved the time.
- The size: the section 4 `cargo bloat --crates` diff, when the change touches generics, dependencies, or profile settings.
- The profiles: `panic` and `strip` did not change in a profile that symbolication or an FFI catch depends on.
- New `unsafe` or new parallelism: checked with `-Zsanitizer=address` or `-Zsanitizer=thread` (the `rust-sanitizers-miri` skill).

## Failure triage

| Symptom | Cause | Fix |
|---------|-------|-----|
| Flamegraph shows hex addresses | The profiled build has no symbols | Build and profile with `--profile profiling`; check it with `readelf -S` |
| `cargo flamegraph` fails, or Linux stacks are wrong, cut off, or nameless | Setup: `perf_event_paranoid`, `--no-rosegment`, frame pointers, `xctrace` permission, or a `--locked` flag it does not take | The failure triage in [references/cargo-flamegraph-setup.md](references/cargo-flamegraph-setup.md) |
| A flag from `.cargo/config.toml` has no effect | `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, or `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` replaces it | Unset it, or move every flag to one config level; check the `rustc` line of `cargo build -v` |
| Frames or reports show raw `_R...` names | The tool predates v0 mangling (default since Rust 1.97) | Upgrade the tool, or pipe text output through `rustfilt` |
| Symbolication shows `<unknown>` | Stripped library | Use the unstripped `.so` from `target/<triple>/<profile>/` |
| Benchmark results swing by more than 10% | Thermal or scheduler noise | Fix the power state, close background load, raise `sample_size` |
| Binary grew after a dependency bump | New monomorphizations or new codegen | `cargo bloat --crates` then `cargo llvm-lines` on the top crate |

## Silent failures

None of these gives an error:

- A CI gate that cannot fail. Criterion prints "Performance has regressed." and exits 0. Gungraun without the main-branch baseline prints `N/A` and passes.
- Unsound parallel code. Never use a JNI environment or a thread-unsafe FFI handle inside a `rayon` closure: it runs on other threads.
- A lost FFI error path. `panic = "abort"` makes every `catch_unwind` inert, so an FFI entry point aborts instead of returning an error.
- Crashes that nobody can symbolicate. A rebuild with other `debug` or `strip` settings has a different build ID.
- A library that faults on the device. Never put `-C target-cpu=native` in a config that cross-compiles: the library assumes host CPU features and faults with an illegal instruction.

## 1. Host profiling

`samply` 0.13.1 samples on macOS and Linux and opens the result in the Firefox Profiler: `samply record ./target/profiling/myapp <args>` (install with `cargo install --locked samply`).

`cargo flamegraph` (`perf` on Linux, `xctrace` on macOS) runs its own build without `--locked`, so run `cargo build --locked --profile profiling --bin myapp` first, then `cargo flamegraph --profile profiling --bin myapp -- <args>`. Read [references/cargo-flamegraph-setup.md](references/cargo-flamegraph-setup.md) before a first run, when stacks look broken, or when you read a flamegraph. Its x86_64 Linux `--no-rosegment` flag belongs in the profiling run only: in the repository config it reaches shipped builds.

For a heap profile, run `heaptrack` on Linux, or the `dhat` crate on any host. Read [references/heap-profiling.md](references/heap-profiling.md) before you add `dhat`: a wrong binding records nothing, and it must stay behind a feature.

## 2. Android on-device profiling

Use `simpleperf`, or Perfetto for scheduler and frame data. The app must be profileable (`<profileable android:shell="true" />`) or debuggable; symbolize against the unstripped `.so` under `target/<triple>/<ship-profile>/`. Read [references/android-profiling.md](references/android-profiling.md) when you record on a device, and its common-mistakes table when a step fails.

## 3. iOS on-device profiling

Use Instruments through Product -> Profile in Xcode. Read [references/ios-profiling.md](references/ios-profiling.md) for the symbol setup, `os_signpost` markers, and MetricKit.

## 4. Binary size (cargo-bloat)

Measure the profile that ships, not `release`. Run `cargo bloat --locked --profile <ship-profile> --target <triple> --crates > before.txt`, again after the change, and diff. The crate whose line grew is the verdict; `-n 20` lists the top functions. An FFI crate at the top points at the enums and records that cross the boundary.

## 5. Monomorphization bloat (cargo-llvm-lines)

Run `cargo llvm-lines --locked --release -p my-crate | head -30`. A high `Copies` count means many instantiations: keep the generic surface and move the body into one concrete inner function ([references/build-time-optimization.md](references/build-time-optimization.md) has the thin-wrapper example).

## 6. Benchmarks

Use Criterion 0.8 by default, and Gungraun (Valgrind instruction counts, Linux only) for a noise-free number or a CI gate. Do not use `#[bench]`: it is nightly-only (E0658 on stable). Declare `harness = false` for every benchmark target, otherwise the built-in test harness intercepts the arguments. Read [references/benchmarking.md](references/benchmarking.md) when you pick another harness (Divan, Hyperfine, Gungraun) or write, review, or gate benchmark code.

Compare with `cargo bench --locked -p my-crate --bench decode -- --save-baseline before`, then `-- --baseline before` after the change. Name the target with `--bench` when you pass Criterion flags: otherwise the lib target's libtest harness gets them too and stops with `error: Unrecognized option: 'save-baseline'`.

Read the `change` interval in Criterion's output, not the midpoint. If the interval crosses zero, or `p > 0.05`, there is no measured change: increase `sample_size` or `measurement_time`, and do not lower the significance level. A low p-value is not proof either (memory-layout variance repeats within one build): confirm a small win with Gungraun.

Prove that the benchmark measured work: run the routine at two input sizes 10x apart. A ratio near 1.0 means LLVM folded the work away, even behind `black_box` (benchmarking.md section 6).

In CI, run `cargo bench --locked --workspace --no-run` or `cargo test --locked --workspace --benches` on every change. Do not gate on Criterion or on wall clock on a shared runner; gate on Gungraun instruction counts against a main-branch baseline with `--callgrind-limits` (benchmarking.md section 7).

## 7. Data-parallel work with rayon

A library must not build the global `rayon` pool: the host, or an earlier `par_iter` call, can create it first, and `build_global` then returns an error. Build a library-owned pool once at init with `rayon::ThreadPoolBuilder::new().num_threads(threads).build()`, return its error to the caller, and run the work inside `pool.install(|| ...)`. On a phone, take the thread count from the host app.

## 8. Profiles and LTO

Set `[profile.*]` at the workspace root; member and dependency tables are ignored.

- `panic`: `"abort"` gives the smallest binary; use `"unwind"` when an FFI entry point must return an error instead of aborting. The `rust-panic-safety` skill owns the boundary policy: with `"unwind"`, guard each entry point with `catch_unwind`.
- `strip`: use `"symbols"` only when the artifact never needs crash symbolication. Otherwise keep `"none"` with `debug = "line-tables-only"`, strip the shipped copy in packaging, and archive the unstripped Cargo output.
- `lto`: a true no-LTO baseline is `lto = "off"`; the default `lto = false` still runs thin-local LTO.
- `opt-level`: `"s"` and `"z"` turn off the loop vectorizer; measure the hot loop at `3` (the `rust-hot-path` skill has the probe).

Read [references/build-configuration.md](references/build-configuration.md) when you change
profiles, rustflags, `target-cpu`, instrumented PGO, or the global allocator.

Read [references/sample-pgo.md](references/sample-pgo.md) for sample capture, conversion,
and `-Cprofile-sample-use` validation.

## 9. Build time

Run `cargo build --locked --timings` first; it writes `target/cargo-timings/cargo-timing.html`. Look for long sequential chains, crates over 10 s, and proc-macro crates that block everything downstream. Then read [references/build-time-optimization.md](references/build-time-optimization.md): sccache, the cross-compilation matrix, workspace splitting, and linker choice.

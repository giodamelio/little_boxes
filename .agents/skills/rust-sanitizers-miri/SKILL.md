---
description: Use when running Miri or a sanitizer (ASan, ThreadSanitizer or TSan, MSan, HWASan) on Rust code with MIRIFLAGS or -Zsanitizer, choosing between Stacked Borrows and Tree Borrows, reading a Miri or ASan report, stubbing FFI that Miri cannot execute, enabling Android MTE (SEGV_MTESERR, SEGV_MTEAERR) or Xcode sanitizers, running UBSan on a C dependency, or wiring these checks into CI. Not for writing or reviewing unsafe code itself; use rust-unsafe.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-sanitizers-miri
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: baee6662514a54e60a99d869b181ffda8680138b
name: rust-sanitizers-miri
---
# Rust Sanitizers and Miri

## 1. Select the tool

| Bug class | Tool | Does not find it |
|---|---|---|
| Aliasing breach, invalid value, provenance error in Rust | Miri | ASan, HWASan, MTE: they do not model Rust rules |
| Heap overflow, use-after-free, double free at runtime | ASan, HWASan or MTE | Miri, when the path reaches foreign code |
| Data race between threads | TSan, or Miri with `-Zmiri-many-seeds` | ASan |
| Read of uninitialized memory | Miri, or MSan with every object instrumented | ASan |
| Integer overflow or null dereference in a C or C++ dependency | UBSan: clang `-fsanitize=undefined` on that dependency | Miri (does not run C); rustc (has no UBSan option) |
| Memory error inside a C or C++ dependency | ASan, HWASan or MTE | Miri |

For a crate that has `unsafe` code and FFI, run both Miri and a sanitizer.

Read `references/miri-ub-patterns.md` ("Coverage decision table") when you
decide which tool runs on each crate, or need overhead figures.

A green run does not prove the absence of undefined behaviour (UB): each tool
checks only the paths the tests execute, and says nothing about stubbed,
skipped, or uninstrumented code.

## 2. Review gates

Apply these before you approve a change that adds or edits `unsafe`.

- [ ] A test exercises the new `unsafe` path.
- [ ] `cargo +nightly miri test --locked` passes on that test, or the test has
      `#[cfg_attr(miri, ignore)]` and a comment that gives the reason.
- [ ] The strict-provenance job passes, or the code exposes provenance on
      purpose and runs in the separate job (section 9).
- [ ] Raw-pointer code also passes under `-Zmiri-tree-borrows`.
- [ ] Byte-parsing code passes under `-Zmiri-symbolic-alignment-check`.
- [ ] Each new `#[cfg(miri)]` stub keeps the signature, the return domain and
      any stored pointer of the real function.
- [ ] An ASan, HWASan or MTE run covers every path that Miri skips.
- [ ] New concurrency is covered by TSan, by Miri with `-Zmiri-many-seeds`, or
      by `loom`.

## 3. Failure triage

Messages are from Miri on nightly 2026-11-09. Match on the quoted fragment.

| Symptom | Probable cause | Next action |
|---|---|---|
| Miri: `unsupported operation: can't call foreign function` | The test reaches code that Miri cannot interpret | Apply section 7 |
| Miri: `not available when isolation is enabled` | The test reads a host resource (`SystemTime::now`, a file) | Add `-Zmiri-disable-isolation` to that job, or inject the value |
| Miri: `allocN has been freed, so this pointer is dangling` | A pointer kept across a reallocation or a drop | Re-derive the pointer after each operation that can reallocate |
| Miri: `in-bounds pointer arithmetic failed` | `add` or `offset` past the end of the allocation | Check the length before the arithmetic |
| Miri: `constructing invalid value` ... `expected a valid enum tag` or `expected a boolean` | A transmute or read made a value no variant or `bool` uses | Decode with `TryFrom` and return an error |
| Miri: `accessing memory based on pointer with alignment` ... `is required`, only on some seeds | A misaligned read through a raw pointer | Use `read_unaligned` or `from_le_bytes`; add `-Zmiri-symbolic-alignment-check` |
| Miri: `memory is uninitialized` or `encountered uninitialized memory` | A read of `MaybeUninit` before every byte was written | Write every byte first, or use a zero-filled buffer |
| Miri: `does not exist in the borrow stack` (SB) or `is forbidden` (TB) | An aliasing violation | Section 6, section 8, and `references/miri-ub-patterns.md` |
| Miri: ``integer-to-pointer casts and `ptr::with_exposed_provenance` are not supported`` | A pointer rebuilt from an integer under `-Zmiri-strict-provenance` | Keep the original pointer, or use `with_addr` or `map_addr`; move required exposure to the separate job |
| Miri: `dangling pointer (it has no provenance)` | A pointer made from an integer that was never exposed | Keep the original pointer, or use `with_addr` |
| Miri: `memory leaked: allocN` | An allocation no static reaches: `Box::leak`, `mem::forget`, a thread still running at exit, or foreign state | Store a process-lifetime value in a `OnceLock` or `LazyLock` static, or add `-Zmiri-ignore-leaks` to that job only |
| A sanitizer build fails: ABI mismatch, undefined `__asan_*` in doctests, rustc aborts in a proc macro; or MSan blames a C dependency | Build setup | Read `references/host-sanitizers.md` ("Build failures") |
| Miri passes, ASan fails | A defect in foreign code that Miri stubbed or skipped | Debug with ASan; read the allocation stack |
| ASan passes, Miri fails | An aliasing or provenance breach that corrupted nothing on this run | Fix it. It is UB, and the optimizer can act on it later |
| TSan reports a race inside an atomics-based structure | An ordering is too weak, or the code synchronizes with a `fence` that TSan does not model | Confirm with Miri `-Zmiri-many-seeds` or `loom` before you change an ordering; see the `memory-model` skill |

## 4. Run a sanitizer on the host

`-Zsanitizer` needs nightly (unstable as of Rust 1.99.1).

```bash
rustup toolchain install nightly --component rust-src
HOST="$(rustc +nightly -vV | sed -n 's/^host: //p')"

# AddressSanitizer (Linux, macOS)
RUSTFLAGS="-Zsanitizer=address" RUSTDOCFLAGS="-Zsanitizer=address" \
    cargo +nightly test --locked -Zbuild-std --target "$HOST"

# TSan: -Zsanitizer=thread. MSan: references/host-sanitizers.md
```

Never silence ``mixing `-Zsanitizer` will cause an ABI mismatch`` with
`-Cunsafe-allow-abi-mismatch=sanitizer`: the mixed build misses errors and can report false
positives. Pass `-Zbuild-std` and `--target "$HOST"`, and set `RUSTDOCFLAGS` to the `RUSTFLAGS`
value. Read `references/host-sanitizers.md` when a sanitizer build fails, before an MSan or TSan
run, or on macOS (ASan needs `ASAN_OPTIONS` there).

## 5. Run Miri

```bash
rustup +nightly component add miri
cargo +nightly miri test --locked                  # whole suite
cargo +nightly miri test --locked <test_filter>    # one test
```

Nightly and cache rules:

- Keep secrets out of any Miri job that writes a `target/` cache a pull request
  can read. Nightlies before 2026-09-22 wrote every environment variable into
  `target/`; if such a job had secrets, clear the cache and rotate them ([Rust blog, 2026-09-21](https://blog.rust-lang.org/2026/09/21/github-actions-leaking-secrets-when-miri-output-is-cached/)).
- Pin a nightly date (2026-09-22 or later) if a Miri regression blocks the
  pipeline.

Miri is slow. Gate a large-input test with `#[cfg_attr(miri, ignore)]`, or
shrink its input under `cfg(miri)`.
Its weak-memory emulation is not complete.

Miri cannot execute a foreign function that has no Miri shim: a `-sys` crate, JNI, UniFFI
scaffolding, inline assembly. `-Zmiri-native-lib` is experimental and unsound; a run through it
is not evidence for the FFI path. Read `references/ffi-stubs.md` ("What Miri can execute") before
you stub a missing shim.

Read `references/miri-ub-patterns.md` when a Miri run fails and you need the
shape of the defect and its fix.

## 6. Aliasing model policy

```bash
cargo +nightly miri test --locked                                  # Stacked Borrows (default)
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test --locked  # Tree Borrows
```

- The default model, Stacked Borrows (SB), is the gate. Fix an SB failure even
  when TB passes: TB accepts some patterns that SB rejects.
- Add a Tree Borrows (TB) run as more evidence on hand-written raw-pointer code.
  Never make TB the only gate: the Miri README calls it "even more experimental
  than Stacked Borrows". Treat a failure under either model as a finding.

## 7. Stub an FFI dependency that Miri cannot execute

- Gate an `extern` block that you declare with `#[cfg(not(miri))]`, and put a
  `#[cfg(miri)]` stub with the same signature next to it. Use the built-in
  `cfg(miri)`, not a Cargo feature.
- Keep the return domain: a stub that returns a pointer returns a pointer into
  a real allocation. A stub stores each pointer that the foreign side stores,
  and dereferences it on a later call.
- Do not stub a third-party C crate or generated bindings. Exclude the crate
  (`--exclude`) or gate the tests that reach it with
  `#[cfg_attr(miri, ignore)]` and a comment with the reason.
- Every path that a stub, a skip or an exclusion removes from Miri needs an ASan
  run on the host, or a HWASan or MTE run on a device.

Read `references/ffi-stubs.md` when you pick the strategy for a dependency or
write a stub.

## 8. A `Box` and a pointer that foreign code keeps

If foreign code stores a `*mut T` taken from a live `Box<T>`, and Rust then
writes through the `Box`, a later foreign access through the stored pointer is
UB. Transfer ownership with `Box::into_raw`, and recover it with
`Box::from_raw` exactly once, after the foreign side unregisters it and no
callback can still run. Read `references/ffi-stubs.md` ("Ownership patterns for
a `Box`") for the rules and `references/miri-ub-patterns.md` ("`Box` plus FFI
aliasing") for stubs that let Miri see it.

## 9. Miri jobs and flag hazards

```bash
# Gate: default model and strict provenance.
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test --locked

# Byte parsers and raw-pointer crates: symbolic alignment, then Tree Borrows.
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" \
    cargo +nightly miri test --locked -p my-parser
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance" \
    cargo +nightly miri test --locked -p my-parser

# Concurrency: many schedules for the threaded tests.
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test --locked <test_filter>

# Endianness: interpret on a big-endian target.
cargo +nightly miri test --locked -p my-parser --target s390x-unknown-linux-gnu
```

Flag hazards: `-Zmiri-strict-provenance` rejects every deliberate exposure, and no `cfg`
reports it, so keep exposing tests out of the gate with `-- --skip <name>` or `--exclude`. Miri
reports 1 CPU by default and so misses a race between two tests; add `-Zmiri-num-cpus=4` for
that job. Read `references/miri-flags-and-ci.md` before you add a flag to a job, or to reproduce
one failing seed with `-Zmiri-seed=N`.

## 10. Read an ASan report

Read frame `#0`, then the allocation and free stacks below it. The `unsafe`
block on the path between them owns the bug. If that path is pure Rust, write
a Miri test for it. Read `references/miri-ub-patterns.md` ("ASan error names")
when you map an ASan error name to its usual Rust cause.

## 11. On-device sanitizers: Android and iOS

On Android arm64, use HWASan or MTE. Xcode does not instrument a prebuilt Rust
static library, so run a host sanitizer build for the Rust code. Read
`references/platform-sanitizers.md` when you build for a device.

## 12. CI integration

Treat a Miri failure as a build failure. Read `references/miri-flags-and-ci.md`
when you write the CI jobs.

## Related skills

When installed: `rust-unsafe` (unsafe patterns), `memory-model` (atomic orderings; an ordering
change also needs a `--cfg loom` test), `rust-debugging` (symbolication), and `rust-test-tools`
(property tests and fuzzing that feed these tools).

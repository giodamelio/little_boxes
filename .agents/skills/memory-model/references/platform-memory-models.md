# Platform Memory Models Reference

Background for the `memory-model` skill: the happens-before rules, the platform
table, fences, the C++ equivalence table, and the case that needs `SeqCst`.

Contents:

- [Happens-before relation](#happens-before-relation)
- [Platform memory models](#platform-memory-models)
- [Fences](#fences)
- [C++ / Rust ordering equivalence](#c--rust-ordering-equivalence)
- [SeqCst total order](#seqcst-total-order)

## Happens-before relation

A **happens-before** (HB) relation between operation A and operation B means:
the effects of A are guaranteed to be visible when B executes.

You establish HB in three ways:

1. **Sequenced-before**: program order inside a single thread.
2. **Synchronizes-with**: a release store synchronizes-with an acquire load
   that reads the value written by that store.
3. **Transitivity**: if A HB B and B HB C, then A HB C.

```text
Thread 1                                Thread 2
DATA.store(42, Relaxed)                 while !READY.load(Acquire) {}
READY.store(true, Release) --sync-with-->   (the load reads `true`)
                                        DATA.load(Relaxed) == 42   // HB guarantees this
```

Consequences:

- A release store publishes only the writes that come **before** it in program
  order. A write placed after the store is not published by it.
- An acquire load covers only the reads that come **after** it in program
  order. A read placed before the load is not covered.
- The relation is per atomic. A release on atomic A does not synchronize with
  an acquire on atomic B.

## Platform memory models

| Platform | Default ordering | Barrier cost | Notes |
|----------|------------------|--------------|-------|
| x86 / x86-64 | TSO (total store order) | Acquire and Release need no extra instruction | SeqCst stores need `mfence` or a locked instruction |
| **ARM64 (Android, Linux, iOS, Apple silicon)** | **Weakly ordered** | **All barriers explicit** | The weakest model of the usual shipping targets; other-multi-copy-atomic since ARMv8 |
| POWER | Weakly ordered | Explicit | Weaker than ARM64: not multi-copy atomic, so two readers can see two independent writes in opposite orders (IRIW) unless every access is `SeqCst` |
| RISC-V | RVWMO | Defined per instruction | Fence granularity is per access type |

- `Relaxed` loads and stores cost no barrier on ARM64 and give no ordering.
- `Acquire` and `Release` cost real instructions on ARM64: the ordered load and
  store forms (`ldar`, `stlr`) or an explicit `dmb` barrier. On x86 plain loads
  and stores already have these semantics.
- x86 keeps loads and stores in order, except that a store can pass a later
  load of a different location (TSO). The compiler also reorders `Relaxed`
  accesses, so a `Relaxed` bug can appear on x86 after an unrelated
  optimization change.
- Apple silicon uses the ARM64 memory model. A macOS ARM64 machine can
  reproduce these bugs.

Use `compare_exchange_weak` only inside a retry loop. On LL/SC targets (ARMv8.0
without LSE, RISC-V) the strong form contains an inner retry loop that the weak
form omits. Where LSE is in the target baseline, both forms compile to the same
instruction.

Compare-exchange cost depends on the target baseline. Measured with
`rustc 1.98.1 -O --emit asm`; the Android, iOS, and `linux-gnu` lines were
re-run on Rust 1.99.1:

- `aarch64-linux-android` and `aarch64-apple-ios` have no LSE in their
  baseline. The strong form is an `ldaxr`/`stlxr` loop, and the weak form is
  one attempt.
- `aarch64-apple-darwin` has LSE in its baseline. Both forms compile to one
  `casal` instruction.
- `aarch64-unknown-linux-gnu` and `aarch64-unknown-linux-musl` have no LSE in
  their baseline, but their target spec enables `+outline-atomics`. Both forms
  compile to the same `bl __aarch64_cas8_acq_rel` call, and the helper selects
  LSE or LL/SC at run time. `--print cfg` does not show this feature. The target
  spec does (`"features": "+v8a,+outline-atomics"`), but printing it needs
  nightly: `rustc +nightly -Z unstable-options --print target-spec-json --target
  <target>`. Stable 1.99.1 rejects `--print target-spec-json` without that flag.
  On stable, emit assembly and look for the helper call:
  `cargo rustc --release --target <target> -- --emit asm`, then search the `.s`
  file for `bl __aarch64_cas`.

The Android and iOS lines are for the default target features. With
`-C target-feature=+outline-atomics`, both forms on those two targets also
compile to the `__aarch64_cas8_acq_rel` call. Rust 1.99.1 warns that this
feature is unstable for `-C target-feature`.

## Fences

A fence orders the operations of the thread that executes it. Use a fence when
the ordering does not belong to one specific atomic access.

| Call | Effect |
|------|--------|
| `fence(Ordering::Release)` | No earlier memory operation moves after the fence. |
| `fence(Ordering::Acquire)` | No later memory operation moves before the fence. |
| `fence(Ordering::AcqRel)` | Both directions. |
| `fence(Ordering::SeqCst)` | Both directions plus participation in the SeqCst total order. |
| `compiler_fence(...)` | Blocks compiler reordering only. Emits no CPU barrier. |

- A fence synchronizes only through an atomic access. `fence(Release)` followed
  by a store to X synchronizes with a load of X that reads that store and is
  followed by `fence(Acquire)`, or that is itself `Acquire`.
- Use `compiler_fence` only for signal handlers and interrupt handlers on the
  same thread, and for a DMA buffer handoff on a single-core MCU with no data
  cache (the `rust-embedded-no-std` skill, when it is installed, has the DMA
  rules). It gives no protection against another core.
- Prefer an ordered atomic operation over a separate fence. A fence is harder to
  review, because the pairing is not visible at the access site.
- ThreadSanitizer does not support `fence`, so it reports false races on
  fence-based code. std `Arc` replaces its `Acquire` fence with an `Acquire`
  load under TSan for this reason. Do not strengthen an ordering to silence such
  a report. Check the code with Miri or loom instead.

## C++ / Rust ordering equivalence

Use this table when you read literature, standards text, or C++ source.

| C++ | Rust | Notes |
|-----|------|-------|
| `memory_order_relaxed` | `Ordering::Relaxed` | |
| `memory_order_acquire` | `Ordering::Acquire` | |
| `memory_order_release` | `Ordering::Release` | |
| `memory_order_acq_rel` | `Ordering::AcqRel` | Read-modify-write only |
| `memory_order_seq_cst` | `Ordering::SeqCst` | |
| `memory_order_consume` | (use `Acquire`) | Compilers already treat consume as acquire |
| `atomic_thread_fence(acquire)` | `fence(Ordering::Acquire)` | |
| `atomic_signal_fence` | `compiler_fence(Ordering::*)` | Compiler barrier only |
| `compare_exchange_strong` | `compare_exchange` | Same rule: the failure ordering is free of the success ordering |

Rust has no `Ordering::Consume`. Map C++ consume code to `Acquire`.

## SeqCst total order

`Ordering::SeqCst` establishes one total order across all SeqCst operations in
all threads. Every thread observes those operations in the same order.

Use SeqCst only when correctness depends on several **different** atomics being
observed in a consistent global order. The classic case is Dekker-style
mutual exclusion: two threads each store to their own flag and then load the
other flag. Acquire-release is not enough there, because a store followed by a
load of a different atomic can be reordered.

If you cannot name the second atomic that needs the shared order, you do not
need SeqCst. Benchmark the cost on a weakly ordered target before you keep it.

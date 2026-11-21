---
description: Use when choosing or reviewing atomic orderings (Ordering::Relaxed, Acquire, Release, AcqRel, SeqCst) or declaring global state (static mut, const vs static, OnceLock, LazyLock, thread_local), including happens-before edges, fences, compare-exchange or fetch_update loops, lock-free code, and a data race seen only on ARM64. Triggers on "global variable", "lazy_static", "once_cell".
license: BSD-3-Clause
metadata:
    github-path: skills/memory-model
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: e9d8ed98cf43b697a3bc125fbf383c75c1a32cc3
name: memory-model
---
# Memory Model

Assume a weakly ordered target (ARM64, RISC-V, POWER). These CPUs reorder
memory accesses that x86 keeps in order. An ordering bug can pass every test on
x86 and fail on a phone or on an Apple silicon Mac.

## Core rules

1. **Derive the ordering from the data it publishes.** Name the writes that the
   reader must see, then pick the weakest ordering that publishes them. Do not
   start from `SeqCst`: it hides the missing reasoning, and it does not repair
   an unpaired or misplaced access.
2. **Ordering is a property of a pair.** A `Release` store synchronizes
   with an `Acquire` load of the same atomic that reads the stored value.
3. **An atomic that publishes nothing needs only `Relaxed`.** Counters and
   unique ids are the usual case.
4. **Prefer a lock, `OnceLock`, or a channel over hand-rolled publication.**
   To publish non-atomic data through a flag you need `UnsafeCell` and
   `unsafe`. Treat that code as a lock-free structure: it needs a loom test and
   an unsafe review (the `rust-unsafe` skill, when it is installed).
5. **A pass on x86 proves nothing about ordering.** The compiler also reorders
   `Relaxed` accesses, so a `Relaxed` bug can appear on x86 after an unrelated
   optimization change. Use the checks in [Verification](#verification).

## Common mistakes

| Mistake | Fix |
|---------|-----|
| `Relaxed` used for publish/subscribe | Use `Release` on the store and `Acquire` on the load. |
| `SeqCst` used everywhere "to be safe" | Derive the ordering from the data dependency. Keep `SeqCst` only for a named multi-atomic order. |
| `Release` on one atomic paired with `Acquire` on a different atomic | Pair the operations on the same atomic, or build an explicit synchronization chain. A `SeqCst` fence alone does not connect different atomics. |
| `static mut` used for shared data | Use `Mutex`, `RwLock`, `OnceLock`, `LazyLock`, or an atomic. |
| `const` used for shared state with interior mutability | Declare it `static`. A `const` is inlined, so every use site mutates a fresh copy. |
| Non-atomic data guarded only by an atomic flag | Guard the payload with a lock. Hand-rolled `UnsafeCell` publication needs a loom test and an unsafe review. |
| Reference count: `Relaxed` decrement, or no overflow abort | `AcqRel` decrement; abort above `isize::MAX`, as `Arc::clone` does. |
| `compare_exchange` on a pointer or index that can be freed and reused (ABA) | Pack a generation counter with the value, or use epoch-based reclamation. |
| Side effect in an `update` or `try_update` closure | Keep the closure free of side effects. It runs again after each lost race. |
| `compiler_fence` used to order memory between threads | Use `fence` or an ordered atomic. `compiler_fence` emits no CPU barrier. |
| ThreadSanitizer reports a race on fence-based code | Do not strengthen the ordering to silence it. TSan does not support `fence`, so the report can be false. Check with Miri or loom. |
| ARM64 assumed to behave like x86 | Run Miri and loom, and test on a weakly ordered host. |

## Verification

Match the check to the risk. Each check misses a class of bugs.

```bash
# Miri: data races and UB on the executed paths, one schedule per seed.
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test --locked <test_filter>

# loom: all interleavings of a loom-instrumented test.
RUSTFLAGS="--cfg loom" cargo test --locked --release --test <loom_target>

# Hardware: the normal suite on an aarch64 host or device.
cargo test --locked
```

- **Miri** runs, when a nightly toolchain with the `miri` component is
  installed, for any change that adds or edits an atomic operation that a test
  reaches. Without it, report the atomic change as not checked by Miri. Each
  seed is one schedule, and Miri emulates only some weak memory effects, so a
  pass is evidence, not proof. Add `MIRIFLAGS="-Zmiri-many-seeds=0..16"`; the
  `rust-sanitizers-miri` skill, when it is installed, owns the other flags and CI.
- **loom** runs for a lock-free structure, a hand-written lock, or publication
  through `UnsafeCell`. It checks only code built on its types: `loom::sync`
  atomics, `loom::thread`, and `loom::cell::UnsafeCell` for the payload. A
  payload in `std::cell::UnsafeCell` is invisible to loom, so gate the cell type
  with `cfg(loom)` too. Run it with `RUSTFLAGS="--cfg loom"`; the
  `rust-test-tools` skill, when it is installed, owns the setup. Know its two limits. It treats `SeqCst` loads and stores as
  `AcqRel`, so a failure in `SeqCst` code can be a false alarm (`fence(SeqCst)`
  is modelled). It does not explore load buffering, so a pass is not proof. A
  `LOOM_MAX_PREEMPTIONS` bound also limits what a pass covers.
- **A weakly ordered host**, such as an Apple silicon Mac, runs the suite
  before you ship a lock-free structure. A pass proves only that run.

## Review checklist

Check each item before you approve a diff that touches atomics.

- [ ] Every `Release` store has a matching `Acquire` load of the same atomic.
- [ ] Every ordering is the weakest one that is still correct, with a comment
      that names the data it publishes.
- [ ] No `SeqCst` remains without a stated multi-atomic ordering requirement.
- [ ] Non-atomic shared data is guarded by a lock, not by the flag alone.
- [ ] A reference count follows its row in [Choosing an ordering](#choosing-an-ordering).
- [ ] Every compare-exchange and `update` failure ordering is `Relaxed`,
      `Acquire`, or `SeqCst`, and is chosen for what the losing thread reads.
- [ ] Every CAS on a pointer or index is safe against ABA.
- [ ] New code uses `update` or `try_update`, not `fetch_update`, unless the
      MSRV is below 1.95.
- [ ] Busy-wait loops call `std::hint::spin_loop()` or are replaced by blocking.
- [ ] The change has a Miri run, or the report says it is not checked by Miri
      because no nightly `miri` component is installed. A lock-free change also
      has a loom test.

## Global state

A global is shared state, so every rule above applies to it. Choose the
declaration form first. Each form fails in a different way.

| Form | Use it for | Cost |
|------|------------|------|
| `const NAME: T` | A compile-time value with no identity: a limit, a table of `&str`. | Inlined at every use site. No address. No state. |
| `static NAME: T` | Shared state with a `const` initializer: an atomic, a `Mutex`. | One address for the process. `T` must be `Sync`. |
| `static NAME: OnceLock<T>` | A value written once at start-up by its owner. | One atomic load per read. `get` returns `Option<&T>`. |
| `static NAME: LazyLock<T>` | A value computed on first use from a fixed initializer. | One atomic load per read. Derefs to `&T`. |
| `thread_local! { static NAME: T }` | Per-thread scratch: a buffer, a recursion depth, an RNG. | No synchronization. One value per thread. |

Read [references/global-state.md](references/global-state.md) before you
declare or review a global, or rewrite a `static mut`: it holds the five rules
behind this table (`Sync` on a `static`, no address for a `const`, `OnceLock`
versus `LazyLock` on panic, no `Drop` for a `static`, thread-local destructor
holes), the `static mut` migration, and a run block with each form.

## Orderings

| Ordering | Valid on | Guarantee |
|----------|----------|-----------|
| `Relaxed` | load, store, read-modify-write | Atomicity and one modification order per atomic. No order for other memory. |
| `Acquire` | load, read-modify-write, fence, compiler_fence | When it reads the value of a `Release` store, every write before that store is visible after it. |
| `Release` | store, read-modify-write, fence, compiler_fence | Publishes every earlier write to an `Acquire` load that reads this value. |
| `AcqRel` | read-modify-write, fence, compiler_fence | `Acquire` on the read half, `Release` on the write half. |
| `SeqCst` | all | Acquire and release, plus one total order over all `SeqCst` operations. |

An invalid ordering panics at run time: a `load` with `Release` or `AcqRel`, a
`store` with `Acquire` or `AcqRel`, a `fence` or `compiler_fence` with
`Relaxed`, and a compare-exchange or `update` failure ordering of `Release` or
`AcqRel`. The rustc lint `invalid_atomic_ordering` is deny-by-default and
rejects these calls at compile time when the ordering is a literal. It cannot
see an ordering that arrives through a variable, so the panic remains the last
line of defence.

## Choosing an ordering

```text
Use case                                                  Ordering
Counter, unique id, or statistic                          Relaxed
Stop flag or progress counter; reader uses nothing else   Relaxed
Stop flag or progress counter; reader then reads state    Release store, Acquire load
Publish data from one thread to another                   Release store, Acquire load
Read-modify-write that both consumes and publishes        AcqRel
Reference count                                           Relaxed increment, abort above isize::MAX;
                                                          Release or AcqRel decrement;
                                                          Acquire (load or fence) before the destructor;
                                                          weak upgrade: CAS loop that refuses 0
Lock word of a hand-written mutex or spinlock             Acquire on the lock CAS (Relaxed on failure);
                                                          Release on unlock
Global order across several different atomics             SeqCst on every participating access
```

The lock row matches the std futex mutex:
`compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed)` to lock and
`swap(UNLOCKED, Release)` to unlock. `SeqCst` is needed only when correctness
depends on two different atomics being seen in one global order. Read
[references/platform-memory-models.md](references/platform-memory-models.md)
when you must justify `SeqCst` with a Dekker-style argument, port C++ atomics,
need the formal happens-before rules, or need the per-target barrier and
compare-exchange cost.

Read [ordering-patterns.md](references/ordering-patterns.md) when you write or review a counter
or unique id, publish/subscribe, a stop flag or progress counter, or a reference count.

## Compare-exchange and update loops

Read [ordering-patterns.md](references/ordering-patterns.md#compare-exchange-and-update-loops)
when you write or review `compare_exchange`, `compare_exchange_weak`, `update`, `try_update`, or
`fetch_update`: it holds the failure-ordering rules, the weak or strong choice, and the
`fetch_update` rename.

## Fences

Prefer an ordered atomic operation over a separate `fence`, because the pairing
of a fence is not visible at the access site. A fence synchronizes only through
an atomic access. Read
[references/platform-memory-models.md](references/platform-memory-models.md#fences)
when you write or review a `fence` or `compiler_fence`.

## Related skills

When installed: `rust-send-sync` (`Send` and `Sync` bounds first),
`rust-async-internals` (tokio tasks and cancellation), and `rust-performance`
(measure the barrier cost before you keep a stronger ordering).

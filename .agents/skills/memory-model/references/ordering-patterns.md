# Ordering patterns

Worked examples for the `memory-model` skill: one example per common ordering
pattern, and the compare-exchange and `update` loop rules. Read only the section that
matches the task.

Contents:

- [Pattern 1: counter or unique id (Relaxed)](#pattern-1-counter-or-unique-id-relaxed)
- [Pattern 2: publish/subscribe (Release store, Acquire load)](#pattern-2-publishsubscribe-release-store-acquire-load)
- [Pattern 3: stop flag and progress counter](#pattern-3-stop-flag-and-progress-counter)
- [Pattern 4: reference counting](#pattern-4-reference-counting)
- [Compare-exchange and update loops](#compare-exchange-and-update-loops)

## Pattern 1: counter or unique id (Relaxed)

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

// Hand out a unique id. No other memory is published with it.
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}
```

`fetch_add` with `Relaxed` is still atomic, so every increment is counted. Only
the order against other memory is dropped. The counter stops being `Relaxed`
when the reader uses it to index a buffer that the writer filled: that buffer is
dependent data.

## Pattern 2: publish/subscribe (Release store, Acquire load)

One thread writes data, then stores a flag. The other thread loads the flag,
then reads the data. The `Release`/`Acquire` pair creates the happens-before
edge that makes the data visible.

```rust,run
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

static DATA: AtomicU64 = AtomicU64::new(0);
static READY: AtomicBool = AtomicBool::new(false);

fn main() {
    thread::scope(|s| {
        s.spawn(|| {
            // Publisher: every write the reader needs comes before the Release store.
            DATA.store(42, Ordering::Relaxed);
            READY.store(true, Ordering::Release);
        });
        s.spawn(|| {
            // Subscriber: the Acquire load that reads `true` makes DATA == 42 visible.
            while !READY.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            assert_eq!(DATA.load(Ordering::Relaxed), 42);
        });
    });
}
```

- Put every published write before the `Release` store. A write after the
  store is not published.
- Put every dependent read after the `Acquire` load. A read before the load is
  not covered.
- Call `std::hint::spin_loop()` in a busy-wait. It is a CPU hint, not a yield
  to the scheduler. Block on a channel, a `Condvar`, or `thread::park` when the
  wait is not short and bounded.

## Pattern 3: stop flag and progress counter

A stop flag that only ends a loop can be `Relaxed`. The worker can see the flag
a few iterations late, and loop teardown accepts that delay. Use
`Release`/`Acquire` as soon as the signaller writes state that the worker reads
after it stops, such as a reason or a new configuration. In review, ask for
`Release`/`Acquire` unless a comment states that the flag publishes nothing.
The dependency is easy to add later and easy to miss.

A polled progress counter follows the same rule: `Relaxed` when the reader only
displays the number, `Release`/`Acquire` when the reader then reads the state
that the number refers to. A progress callback or a channel needs no ordering
choice. The `ffi-error-progress-cancel` skill, when it is
installed, covers cancellation and progress across a foreign function interface; its
rule is one cancel path per bridge, where only `awaitClose` (Kotlin) and
`onTermination` (Swift) call `cancelJob`.

## Pattern 4: reference counting

```rust,run
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_REFCOUNT: usize = isize::MAX as usize;

// Increment from a live reference: no data is published, and the object is
// already alive, so `Relaxed` is enough.
fn increment(count: &AtomicUsize) {
    let old = count.fetch_add(1, Ordering::Relaxed);
    // `mem::forget` of a clone is safe code. Without this check a loop of
    // clone-and-forget wraps the count to 0, and the next decrement frees a
    // live object: a use-after-free with no `unsafe` in the caller.
    if old > MAX_REFCOUNT {
        process::abort();
    }
}

// Upgrade from a weak handle: the count can already be 0, and a `fetch_add`
// would revive a destroyed object. Use a CAS loop that refuses 0.
fn try_upgrade(count: &AtomicUsize) -> bool {
    let mut current = count.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            return false;
        }
        if current > MAX_REFCOUNT {
            process::abort();
        }
        match count.compare_exchange_weak(current, current + 1, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

// Decrement: AcqRel, so the thread that observes zero also observes every
// write made by every other owner before it dropped its reference.
fn decrement(count: &AtomicUsize) -> bool {
    count.fetch_sub(1, Ordering::AcqRel) == 1 // true: destroy the object here
}

fn main() {
    let count = AtomicUsize::new(1);
    increment(&count);
    assert!(!decrement(&count));
    assert!(decrement(&count));
    assert!(!try_upgrade(&count)); // the last owner is gone
}
```

The destructor reads the object, so the decrement must acquire the writes of
all other owners. std `Arc` uses a `Release` decrement and a `fence(Acquire)` on
the zero path. Both forms are correct.

The overflow check is required, not defensive. std `Arc::clone` aborts when the
old count is above `isize::MAX`, and `Weak::upgrade` panics there; the check
runs after the `fetch_add`, which leaves `isize::MAX` of headroom before the
`usize` wraps (Rust 1.99.1 `alloc/src/sync.rs`). Apply the same limit to every
increment path, the CAS loop included: one unchecked path can bring the count
near the limit for the other to wrap. `Weak::upgrade` uses `Acquire` on success
so that it synchronizes with `Arc::new_cyclic`, which initializes the value after
`Weak` handles exist.

## Compare-exchange and update loops

```rust,run
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    let val = AtomicUsize::new(0);

    // Strong CAS: one attempt, no spurious failure. The failure ordering is
    // chosen independently of the success ordering (Rust 1.64 and later).
    assert_eq!(val.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Acquire), Ok(0));

    // Weak CAS: it can fail spuriously, so use it only in a loop that feeds the
    // observed value back into the next attempt.
    let mut current = val.load(Ordering::Relaxed);
    loop {
        match val.compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }

    // `update` (1.95+) writes that loop and returns the previous value.
    assert_eq!(val.update(Ordering::AcqRel, Ordering::Relaxed, |v| v + 1), 2);

    // `try_update` (1.95+) stops without a write when the closure returns None.
    assert_eq!(val.try_update(Ordering::AcqRel, Ordering::Relaxed, |v| v.checked_sub(10)), Err(3));
    assert_eq!(val.load(Ordering::Relaxed), 3);
}
```

- **Failure ordering.** It can be `Relaxed`, `Acquire`, or `SeqCst`. `Release`
  and `AcqRel` are invalid, because a failed exchange writes nothing. Since Rust
  1.64 the failure ordering can be stronger than the success ordering. Do not
  weaken a failure ordering only to match the success ordering.
- **Choose each ordering for its own path.** The success ordering covers what
  the winner publishes or consumes. The failure ordering covers what the loser
  reads next. `Relaxed` on failure is correct when the loser only retries.
- **Weak or strong.** Use `compare_exchange_weak` only inside a retry loop.
  Read
  [platform-memory-models.md](platform-memory-models.md#platform-memory-models)
  when you choose between the weak and strong form on an LL/SC target or in a
  hot loop.
- **`fetch_update` is deprecated since Rust 1.99.** Use `update` (the closure
  always returns a value) or `try_update` (the closure returns `Option`). Both
  are stable since 1.95. `try_update` has the same arguments and result as
  `fetch_update`, so rename the call. On 1.98 only the allow-by-default
  `deprecated_in_future` lint reports it. On 1.99.1 every call emits a
  `deprecated` warning ("renamed to `try_update` for consistency"), which fails
  a `-D warnings` gate. Keep `fetch_update` only while `rust-version` is below
  1.95, and put `#[allow(deprecated)]` with a reason on that call.

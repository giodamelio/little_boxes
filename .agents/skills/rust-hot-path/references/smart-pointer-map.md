# Map smart pointers without an assumed allocation count

Contents: select the operation; measure allocation calls; check ownership and panic;
interpret the result.

## Select the operation

Rust 1.100 stabilizes `Box::map`, `Rc::map`, and `Arc::map`. Each consumes the
input pointer and returns a pointer to the transformed value. The closure receives
`T` for `Box`, but `&T` for `Rc` and `Arc`. A shared observer keeps the old value;
this is not an in-place mutation visible through every clone.

Call these as associated functions, for example `Arc::map(value, transform)`.
The contract permits allocation reuse; it does not promise zero allocations.
Check the MSRV before use. The similarly named `try_map` remains nightly-only
in Rust 1.100; a fallible transform needs a separate reviewed design on stable.

Read the uniqueness and layout conditions as implementation facts, not API
invariants. Measure the workload with extra strong owners, weak owners, and
changes to the output size or alignment. Do not remove an owner just to get a
better allocation count if that changes program behavior.

## Measure allocation calls

This probe uses a process-local counting allocator for an otherwise single-threaded
program. Construct inputs before the measured window. Keep printing and assertions
outside it. `black_box` makes accidental removal of the measured operation less
likely; it is not a language guarantee that allocations occur.

```rust,run
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::mem::DropGuard;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

#[repr(align(64))]
struct Aligned([u64; 32]);

struct Meter;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Every operation forwards its original pointer and layout to System.
// The counters use non-allocating atomics and cannot unwind.
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Relaxed) { CALLS.fetch_add(1, Relaxed); }
        // SAFETY: The caller supplies a valid nonzero allocation layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Relaxed) { CALLS.fetch_add(1, Relaxed); }
        // SAFETY: The caller supplies a valid nonzero allocation layout.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ACTIVE.load(Relaxed) { CALLS.fetch_add(1, Relaxed); }
        // SAFETY: The caller supplies a live System block and valid new size.
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller returns a live System block with its layout.
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Meter = Meter;

fn measured<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = CALLS.load(Relaxed);
    ACTIVE.store(true, Relaxed);
    let guard = DropGuard::new((), |_| ACTIVE.store(false, Relaxed));
    let result = black_box(f());
    drop(guard);
    (result, CALLS.load(Relaxed) - before)
}

fn main() {
    // This process starts no worker threads. Construct inputs before counting.
    let b = black_box(Box::new(7_u64));
    let (b, box_same) = measured(|| Box::map(b, |n| n + 1));
    assert_eq!(*b, 8);
    let (b, box_larger) = measured(|| Box::map(b, |n| Aligned([n; 32])));
    assert_eq!(b.0, [8; 32]);

    let r = black_box(Rc::new(7_u64));
    let (r, rc_unique) = measured(|| Rc::map(r, |n| n + 1));
    assert_eq!(*r, 8);
    let observer = Rc::clone(&r);
    let (r, rc_shared) = measured(|| Rc::map(r, |n| n + 1));
    assert_eq!((*observer, *r), (8, 9));
    let weak = Rc::downgrade(&r);
    let (r, rc_weak) = measured(|| Rc::map(r, |n| n + 1));
    assert_eq!(*r, 10);
    assert!(weak.upgrade().is_none());

    let a = black_box(Arc::new(7_u64));
    let (a, arc_unique) = measured(|| Arc::map(a, |n| n + 1));
    assert_eq!(*a, 8);
    let observer = Arc::clone(&a);
    let (a, arc_shared) = measured(|| Arc::map(a, |n| n + 1));
    assert_eq!((*observer, *a), (8, 9));
    let weak = Arc::downgrade(&a);
    let (a, arc_weak) = measured(|| Arc::map(a, |n| n + 1));
    assert_eq!(*a, 10);
    assert!(weak.upgrade().is_none());
    let (a, arc_larger) = measured(|| Arc::map(a, |n| [*n; 32]));
    assert_eq!(*a, [10; 32]);

    println!("allocation calls: Box same={box_same} larger={box_larger}; \
        Rc unique={rc_unique} shared={rc_shared} weak={rc_weak}; \
        Arc unique={arc_unique} shared={arc_shared} weak={arc_weak} larger={arc_larger}");
}
```

On Rust 1.100.0, aarch64 macOS, the observed call counts are `Box` 0/1,
`Rc` 0/1/1, and `Arc` 0/1/1/1 in the printed order. The larger `Box` output also
raises alignment to 64. These are measurements for this probe, not a portable
allocation contract. The probe asserts values and observer behavior, and prints
counts so a changed implementation does not masquerade as a semantic failure.
Set a separate workload budget after measuring the final release build.

## Check ownership and panic

A mapping closure can panic after the pointer has been consumed. Check destruction
on the unwinding profile as well as returned values. For shared pointers, a surviving
observer still owns the source value. The macro below repeats the same independent
cases for `Rc` and `Arc`; it does not start threads.

```rust,run
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

struct Value<'a> { number: u64, drops: &'a Cell<usize> }
impl Drop for Value<'_> {
    fn drop(&mut self) { self.drops.set(self.drops.get() + 1); }
}

// Rc and Arc have the same borrowed-closure cases. Each expansion is isolated.
macro_rules! check_shared_pointer {
    ($pointer:ident) => {{
        let drops = Cell::new(0);
        let input = $pointer::new(Value { number: 7, drops: &drops });
        let output = $pointer::map(input, |v| Value { number: v.number + 1, drops: v.drops });
        assert_eq!((output.number, drops.get()), (8, 1));
        drop(output);
        assert_eq!(drops.get(), 2);

        let drops = Cell::new(0);
        let input = $pointer::new(Value { number: 7, drops: &drops });
        let observer = $pointer::clone(&input);
        let output = $pointer::map(input, |v| Value { number: v.number + 1, drops: v.drops });
        assert_eq!((observer.number, output.number, drops.get()), (7, 8, 0));
        drop(output);
        assert_eq!(drops.get(), 1);
        drop(observer);
        assert_eq!(drops.get(), 2);

        let drops = Cell::new(0);
        let input = $pointer::new(Value { number: 7, drops: &drops });
        let weak = $pointer::downgrade(&input);
        let output = $pointer::map(input, |v| Value { number: v.number + 1, drops: v.drops });
        assert!(weak.upgrade().is_none());
        assert_eq!((output.number, drops.get()), (8, 1));
        drop(output);
        drop(weak);
        assert_eq!(drops.get(), 2);

        let drops = Cell::new(0);
        let input = $pointer::new(Value { number: 7, drops: &drops });
        let result = catch_unwind(AssertUnwindSafe(|| {
            $pointer::map(input, |_| -> Value<'_> { panic!("map probe") })
        }));
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);

        let drops = Cell::new(0);
        let input = $pointer::new(Value { number: 7, drops: &drops });
        let observer = $pointer::clone(&input);
        let result = catch_unwind(AssertUnwindSafe(|| {
            $pointer::map(input, |_| -> Value<'_> { panic!("shared map probe") })
        }));
        assert!(result.is_err());
        assert_eq!((observer.number, drops.get()), (7, 0));
        drop(observer);
        assert_eq!(drops.get(), 1);
    }};
}

fn main() {
    let drops = Cell::new(0);
    let input = Box::new(Value { number: 7, drops: &drops });
    let output = Box::map(input, |mut value| { value.number += 1; value });
    assert_eq!((output.number, drops.get()), (8, 0));
    let output = Box::map(output, |value| value.number);
    assert_eq!((*output, drops.get()), (8, 1));
    drop(output);
    assert_eq!(drops.get(), 1);

    let drops = Cell::new(0);
    let input = Box::new(Value { number: 7, drops: &drops });
    let result = catch_unwind(AssertUnwindSafe(|| {
        Box::map(input, |_| -> Value<'_> { panic!("box map probe") })
    }));
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);

    check_shared_pointer!(Rc);
    check_shared_pointer!(Arc);
}
```

## Interpret the result

Run both examples with `cargo run --locked --example <name>`. Also measure the
counting example with `--release`. The lifecycle probe checks one destruction per
constructed value on success and panic, plus strong and weak observer behavior.
It does not prove synchronization, real workload speed, or cleanup under abort.
The counting allocator is test instrumentation; keep it out of a library build.

Sources: [Box::map](https://doc.rust-lang.org/1.100.0/std/boxed/struct.Box.html#method.map),
[Rc::map](https://doc.rust-lang.org/1.100.0/std/rc/struct.Rc.html#method.map),
[Arc::map](https://doc.rust-lang.org/1.100.0/std/sync/struct.Arc.html#method.map).

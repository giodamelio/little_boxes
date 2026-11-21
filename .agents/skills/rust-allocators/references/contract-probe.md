# Allocator adapter contract probe

Contents: adapter boundary; executable probe; evidence limits.

## Adapter boundary

This Rust 1.100 probe wraps `System` and can reject a request. It implements only
`allocate` and `deallocate`, so the standard `Allocator` defaults perform growth,
shrinkage, and zeroing. The adapter owns no inline memory and uses no global
allocator replacement. Its counters count successful allocator calls, including
zero-sized requests; they are not a measurement of physical heap allocations.

The borrowed allocator outlives the `Vec`. `Cell` also prevents this adapter from
claiming shared access across threads. Saturating counters keep instrumentation
from introducing an unwind inside allocator methods.

## Executable probe

```rust,run
use std::alloc::{AllocError, Allocator, Layout, System};
use std::cell::Cell;
use std::ptr::NonNull;

#[derive(Default)]
struct Budget {
    reject: Cell<bool>,
    allocated: Cell<usize>,
    freed: Cell<usize>,
}

// SAFETY: Successful requests and every deallocation use System with unchanged
// layouts. The adapter owns no backing storage. Moving it cannot move a block.
// Failure leaves all live blocks untouched. Cells and saturating arithmetic
// do not allocate or unwind; this type has no destructor.
unsafe impl Allocator for Budget {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if self.reject.get() {
            return Err(AllocError);
        }
        let block = System.allocate(layout)?;
        self.allocated.set(self.allocated.get().saturating_add(1));
        Ok(block)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.freed.set(self.freed.get().saturating_add(1));
        // SAFETY: The caller returns a live block from this adapter with a
        // fitting layout; every successful request came from System.
        unsafe { System.deallocate(ptr, layout) };
    }
}

fn main() {
    let allocator = Budget::default();
    let small = Layout::from_size_align(8, 64).unwrap();
    let large = Layout::from_size_align(32, 64).unwrap();
    let block = allocator.allocate(small).unwrap();
    let ptr = block.cast::<u8>();
    assert!(block.len() >= small.size());
    assert_eq!(ptr.as_ptr().addr() % small.align(), 0);
    // SAFETY: This live block holds at least eight writable bytes.
    unsafe { ptr.as_ptr().write_bytes(0x5a, small.size()) };

    allocator.reject.set(true);
    assert!(allocator.allocate(small).is_err());
    // SAFETY: ptr is live, small fits it, and large is at least as large.
    assert!(unsafe { allocator.grow(ptr, small, large) }.is_err());
    // SAFETY: A failed grow leaves the old allocation live and unchanged.
    assert_eq!(unsafe { std::slice::from_raw_parts(ptr.as_ptr(), 8) }, &[0x5a; 8]);
    assert_eq!(allocator.freed.get(), 0);

    allocator.reject.set(false);
    // SAFETY: The same old block is still live and large grows its layout.
    let grown = unsafe { allocator.grow_zeroed(ptr, small, large) }.unwrap();
    let ptr = grown.cast::<u8>();
    // SAFETY: grow_zeroed retains the old eight bytes and initializes the rest.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.as_ptr(), 32) };
    assert_eq!(&bytes[..8], &[0x5a; 8]);
    assert_eq!(&bytes[8..], &[0; 24]);
    assert_eq!(ptr.as_ptr().addr() % 64, 0);
    // The old ptr is invalid after successful growth, even if addresses match.

    allocator.reject.set(true);
    // SAFETY: large fits the live grown block, and small is no larger.
    assert!(unsafe { allocator.shrink(ptr, large, small) }.is_err());
    // SAFETY: Failed shrink also retains the original live block.
    assert_eq!(unsafe { *ptr.as_ptr() }, 0x5a);
    allocator.reject.set(false);
    // SAFETY: The same grown block is live and the requested layout is smaller.
    let shrunk = unsafe { allocator.shrink(ptr, large, small) }.unwrap();
    let ptr = shrunk.cast::<u8>();
    assert_eq!(ptr.as_ptr().addr() % 64, 0);
    // SAFETY: shrink preserves the first small.size() initialized bytes.
    assert_eq!(unsafe { std::slice::from_raw_parts(ptr.as_ptr(), 8) }, &[0x5a; 8]);
    // SAFETY: This live small block can grow to the large layout.
    let grown = unsafe { allocator.grow(ptr, small, large) }.unwrap();
    let ptr = grown.cast::<u8>();
    // SAFETY: grow preserves the initialized prefix; the suffix is uninitialized.
    assert_eq!(unsafe { std::slice::from_raw_parts(ptr.as_ptr(), 8) }, &[0x5a; 8]);
    // SAFETY: Return this live block once, with the new fitting layout.
    unsafe { allocator.deallocate(ptr, large) };

    let zero = Layout::from_size_align(0, 64).unwrap();
    let empty = allocator.allocate(zero).unwrap();
    assert_eq!(empty.cast::<u8>().as_ptr().addr() % 64, 0);
    // SAFETY: A zero-sized block is still returned with its matching layout.
    unsafe { allocator.deallocate(empty.cast(), zero) };

    let zeros = allocator.allocate_zeroed(small).unwrap();
    // SAFETY: allocate_zeroed initialized all requested bytes.
    assert_eq!(unsafe { std::slice::from_raw_parts(zeros.cast::<u8>().as_ptr(), 8) }, &[0; 8]);
    // SAFETY: Return this live block once with a fitting layout.
    unsafe { allocator.deallocate(zeros.cast(), small) };

    {
        let mut values = Vec::with_capacity_in(4, &allocator);
        values.extend([1_u32, 2, 3, 4]);
        allocator.reject.set(true);
        assert!(values.try_reserve_exact(1).is_err());
        assert_eq!(values.as_slice(), &[1, 2, 3, 4]);
        allocator.reject.set(false);
    }
    assert_eq!(allocator.allocated.get(), allocator.freed.get());
}
```

## Evidence limits

Run this example with `cargo run --locked --example <name>` on Rust 1.100 or later.
The probe checks both resize outcomes, data preservation, zeroing, alignment,
zero-size handling, balanced returns, and a collection's failure path. Run a copy
against an allocator's overridden methods when those are the code under review.

A balanced counter does not establish pointer provenance, absence of overlap,
thread safety, or a region-reset contract. Use code review and Miri on a compatible
nightly for those memory invariants; use the real foreign boundary for FFI.

Source: [Allocator methods and safety](https://doc.rust-lang.org/1.100.0/std/alloc/trait.Allocator.html).

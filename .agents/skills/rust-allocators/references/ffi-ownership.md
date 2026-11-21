# Allocator identity across raw ownership and FFI

Use `Box::into_raw_with_allocator` or `Vec::into_raw_parts_with_allocator` when
ownership leaves a Rust collection. Keep the allocator with the pointer and, for
`Vec`, the length and capacity. Reconstruct once with the matching `_in` function.
Plain `Box::from_raw` uses the global allocator. It is not a general inverse for
a custom allocation, even when two allocators happen to use the same C backend.

## Keep the foreign contract explicit

- Keep the Rust allocator and its backing region inside an opaque Rust owner.
  Export a C-compatible handle or descriptor, not the Rust layout of `Vec`, `Box`,
  or an allocator trait object.
- Give that owner one release operation implemented by Rust. The foreign caller
  returns the handle to it instead of calling C `free` or its language allocator.
- State who owns the buffer before transfer, after success, and after each failure.
  Transfer success consumes the old owner's release responsibility exactly once.
- State whether the foreign side may write, retain, resize, or release the buffer
  from another thread. An allocator does not establish `Send` or `Sync` for its data.
- Retain the allocator and region until release. A borrowed allocator cannot outlive
  its Rust owner merely because a raw pointer crossed the boundary.
- Validate untrusted lengths and capacities before constructing Rust slices. Only
  reconstruct raw parts that came from the matching allocation and ownership path;
  bounds checks cannot establish the provenance of an arbitrary foreign pointer.

## Round-trip probe

This checks the Rust ownership pair. It does not export an ABI or test a foreign
runtime's release path.

```rust,run
use std::alloc::System;

fn main() {
    let mut values = Vec::with_capacity_in(4, System);
    values.extend([10_u32, 20]);
    let (ptr, len, cap, allocator) = values.into_raw_parts_with_allocator();
    // SAFETY: These parts and this allocator came from the same live Vec.
    // No other owner frees or accesses the allocation during reconstruction.
    let values = unsafe { Vec::from_raw_parts_in(ptr, len, cap, allocator) };
    assert_eq!(values.as_slice(), &[10, 20]);

    let value = Box::new_in(7_u32, System);
    let (ptr, allocator) = Box::into_raw_with_allocator(value);
    // SAFETY: The pointer and allocator came from this Box; ownership is unique.
    let value = unsafe { Box::from_raw_in(ptr, allocator) };
    assert_eq!(*value, 7);
}
```

Run `cargo run --locked --example <name>` for the round trip. At the real boundary,
test success, failure before transfer, failure after transfer, and final release.
Instrument the owner to show that every successful transfer is released once.
Test teardown and callback races when the foreign side can retain the handle.

Sources: [Box allocator-aware raw ownership](https://doc.rust-lang.org/1.100.0/std/boxed/struct.Box.html#method.into_raw_with_allocator),
[Vec allocator-aware raw ownership](https://doc.rust-lang.org/1.100.0/std/vec/struct.Vec.html#method.into_raw_parts_with_allocator).

# Counting allocator

The allocator behind every allocation count in [SKILL.md](../SKILL.md) and its references. Put it
in a bench binary, not in a library: a `#[global_allocator]` in a library replaces the allocator
of every binary that links it. Read the two counters before and after the code under test, and
run the binary in release.

```rust,run
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub static ALLOCS: AtomicUsize = AtomicUsize::new(0);
pub static BYTES: AtomicUsize = AtomicUsize::new(0);

pub struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn main() {
    let (a0, b0) = (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    let cap = std::hint::black_box(Vec::<u32>::with_capacity(1000)).capacity();
    let allocs = ALLOCS.load(Ordering::Relaxed) - a0;
    let bytes = BYTES.load(Ordering::Relaxed) - b0;
    println!("allocs={allocs} bytes={bytes}");
    assert_eq!((allocs, bytes, cap), (1, 4000, 1000));
}
```

`realloc` counts as one allocation and adds the new size, so a growth step shows up in both
counters. `dealloc` counts nothing.

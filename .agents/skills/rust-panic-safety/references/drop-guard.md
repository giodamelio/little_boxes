# Rollback with DropGuard

`core::mem::DropGuard` is stable since Rust 1.100. Its closure consumes the
protected value when the guard drops. `DropGuard::dismiss` returns that value
without running the closure. Commit only after every fallible step succeeds.

This probe checks success, early error, and unwinding. The rollback resets a
counter; a real transaction must also restore every associated resource.

```rust,run
use std::cell::Cell;
use std::mem::DropGuard;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn update(counter: &Cell<u32>, fail: bool) -> Result<(), &'static str> {
    let old = counter.get();
    let guard = DropGuard::new(counter, move |c| c.set(old));
    guard.set(old + 1);
    if fail {
        return Err("update failed");
    }
    DropGuard::dismiss(guard);
    Ok(())
}

fn main() {
    let counter = Cell::new(5);
    assert_eq!(update(&counter, true), Err("update failed"));
    assert_eq!(counter.get(), 5);
    assert_eq!(update(&counter, false), Ok(()));
    assert_eq!(counter.get(), 6);

    // The guard restores the counter before catch_unwind returns.
    let result = catch_unwind(AssertUnwindSafe(|| {
        let guard = DropGuard::new(&counter, |c| c.set(6));
        guard.set(99);
        panic!("rollback probe");
    }));
    assert!(result.is_err());
    assert_eq!(counter.get(), 6);
}
```

Run this as an example with `cargo run --locked --example <name>` in an
unwinding profile. It does not prove
cleanup on abort, process exit, or `mem::forget`, which skip destructors.
Keep cleanup free of panics. A guard is not a memory-safety proof: safe code can
forget it. Use a local `Drop` type when the package MSRV is below 1.100.

The `rust-async-internals` skill, when installed, owns the cancellation case.
Poll the real future to `Pending`, drop its owner, and check rollback before
claiming cancel safety. `DropGuard` runs synchronous cleanup; a remote or async
rollback needs an explicit completion path that the caller awaits.

Source: [DropGuard](https://doc.rust-lang.org/1.100.0/core/mem/struct.DropGuard.html).

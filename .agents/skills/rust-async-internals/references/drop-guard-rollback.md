# Synchronous rollback when a future is dropped

Contents: choose the guard; executable cancellation probe; evidence limits.

## Choose the guard

Use `core::mem::DropGuard` on Rust 1.100+ for a synchronous state rollback. Its
closure runs when the guard is dropped; `dismiss` commits the state without
running that closure. A guard created inside an async body does not exist until
the future is polled. A guard already captured by a future has a different
before-first-poll lifetime; test the actual construction boundary.

`CancellationToken::drop_guard()` instead requests cooperative cancellation.
Dropping it does not wait for tasks to observe the token or finish cleanup.
Retain and join the tasks when completion matters. Neither guard makes an async
rollback or remote transaction finish: perform async cleanup through an explicit
awaited operation or an owned supervisor with a completion signal.

The `rust-panic-safety` skill, when installed, owns general rollback policy;
keep the `DropGuard` closure free of panics and dismiss it only after commit.

## Executable cancellation probe

This std-only probe polls the real async body. It needs no runtime or timer.
It drops the owning `Box` after `Pending`, not a `Pin<&mut _>` reborrow.
The ready flag is changed explicitly because this is a controlled poll sequence,
not an executor example.

```rust,run
use std::cell::Cell;
use std::future::{Future, poll_fn};
use std::mem::DropGuard;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::{Context, Poll, Waker};

#[derive(Clone, Copy)]
enum Exit { Success, Error, Panic }

async fn update(
    state: &Cell<u32>,
    cleanups: &Cell<u32>,
    ready: &Cell<bool>,
    exit: Exit,
) -> Result<(), &'static str> {
    let old = state.get();
    let guard = DropGuard::new(state, move |state| {
        state.set(old);
        cleanups.set(cleanups.get() + 1);
    });
    guard.set(old + 1);
    if matches!(exit, Exit::Error) { return Err("rejected"); }
    poll_fn(|_| if ready.get() { Poll::Ready(()) } else { Poll::Pending }).await;
    if matches!(exit, Exit::Panic) { panic!("rollback probe"); }
    DropGuard::dismiss(guard);
    Ok(())
}

fn main() {
    let state = Cell::new(5);
    let cleanups = Cell::new(0);
    let ready = Cell::new(false);
    let mut context = Context::from_waker(Waker::noop());

    // An async body has not run before its first poll: no mutation or guard.
    let future = Box::pin(update(&state, &cleanups, &ready, Exit::Success));
    drop(future);
    assert_eq!((state.get(), cleanups.get()), (5, 0));

    let mut future = Box::pin(update(&state, &cleanups, &ready, Exit::Success));
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert_eq!((state.get(), cleanups.get()), (6, 0));
    drop(future); // Drop the owner, not only a Pin<&mut _> reborrow.
    assert_eq!((state.get(), cleanups.get()), (5, 1));

    let mut future = Box::pin(update(&state, &cleanups, &ready, Exit::Error));
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(Err("rejected")));
    drop(future);
    assert_eq!((state.get(), cleanups.get()), (5, 2));

    let mut future = Box::pin(update(&state, &cleanups, &ready, Exit::Success));
    assert!(future.as_mut().poll(&mut context).is_pending());
    ready.set(true);
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(Ok(())));
    drop(future);
    assert_eq!((state.get(), cleanups.get()), (6, 2));

    let mut future = Box::pin(update(&state, &cleanups, &ready, Exit::Panic));
    let result = catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(&mut context)));
    assert!(result.is_err());
    drop(future);
    assert_eq!((state.get(), cleanups.get()), (6, 3));
}
```

## Evidence limits

Run `cargo run --locked --example <name>` with an unwinding profile. The assertions
check an unpolled drop, a pending drop, early error, commit, unwind, and exactly one
rollback on each unsuccessful started operation. A committed operation keeps its
new state and does not run rollback.

For a real async operation, repeat the pending-drop test at every suspension point
that can follow a partial state change. Check all associated resources, not only
one counter. This probe proves no remote rollback, runtime shutdown behavior, or
cleanup under abort, process exit, or `mem::forget`.

Source: [DropGuard](https://doc.rust-lang.org/1.100.0/core/mem/struct.DropGuard.html).

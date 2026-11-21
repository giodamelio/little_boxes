# Ownership and concurrency traps

Read this file when a diff picks a builder receiver or passes a large value by value, derives
`Clone` on a handle, shares state through `Arc`, `Weak`, a mutex, or a `Condvar`, or does integer
arithmetic on input. Each trap compiles, and the defect appears later at run time or in a
different build profile. Trait and lifetime traps live in
[type-and-trait-traps.md](type-and-trait-traps.md).

Contents:

- Value-passing performance trap
- `#[derive(Clone)]` on resource-backed types
- `parking_lot` and `tokio` mutexes do not poison on panic
- A bare `Condvar::wait` misses an early notify
- Integer overflow panics in debug and wraps in release
- `Arc` reference cycles without `Weak` leak permanently
- A `Weak` registry never reaps dead slots

---

## Value-passing performance trap

**Severity: WARNING on hot paths**

`fn(T) -> T` copies the value in and out at every call that is not inlined. Above the target's
inline-copy boundary, each such move is a `memcpy` call. When the optimizer inlines the callee, it
can remove the copies, so the cost depends on an inlining decision that you do not control and
that changes between builds. Measured on x86_64-unknown-linux-gnu with rustc 1.98.1 `-O`, a loop
of `s = transform(s)` on the 1032-byte struct below has no `memcpy` inside the loop when
`transform` is inlined, and two `memcpy` calls per iteration under `#[inline(never)]`. Measure
the shipped build; do not assume either result. The `rust-hot-path` skill, when it is installed,
has the probe; at `-O`, x86_64 copies up to 128 bytes inline and aarch64-apple-darwin up to 256.

```rust
struct BigState {
    payload: [u8; 1024],
    counter: u64,
}

// BAD on a hot path: a memcpy in and a memcpy out whenever the call is not inlined
fn transform(mut state: BigState) -> BigState {
    state.counter += 1;
    state
}
```

```rust
// GOOD on a hot path: in-place mutation
struct BigState {
    payload: [u8; 1024],
    counter: u64,
}

fn transform(state: &mut BigState) {
    state.counter += 1;
}
```

Do not keep the `fn(T) -> T` shape through a `take_mut`-style `ptr::read` + closure + `ptr::write`
helper. That helper adds a copy, and a panic inside the closure aborts the process.

Use `fn(T) -> T` only when the ownership transfer is the semantic, as in a state-machine transition
or a builder method `fn set_foo(mut self) -> Self`, or when the type stays under the boundary on
every target you ship.

The builder receiver shape is the case where the choice is not about copy cost. The three shapes
are not interchangeable, and the receiver decides where the builder can live:

| Setter receiver | Chains | Survives a loop | Terminal builder method |
| --- | --- | --- | --- |
| `fn set(&mut self)` returning `()` | no | yes | `fn build(self) -> T` |
| `fn set(mut self) -> Self` | yes | yes, with reassignment | `fn build(self) -> T` |
| `fn set(&mut self) -> &mut Self` | yes | yes | `fn build(self) -> T` from a bound builder |

An owned `self -> Self` setter moves the builder on every call. Keep its returned value with
`builder = builder.set(value)` in a loop or conditional branch. Discarding that value causes
E0382 at the next use. A `&mut self -> &mut Self` setter needs no reassignment and keeps
`fn build(self) -> T`. Only the start of the chain constrains the terminal method. Bind the
builder to a variable and the owned build compiles. Start the chain from a temporary, as in
`CfgBuilder::default().name("a").build()`, and it fails with `error[E0507]: cannot move out of a
mutable reference`, with the note `'CfgBuilder::build' takes ownership of the receiver 'self',
which moves value`. The third failure — an owned setter called on a *field* from a `&mut self`
method — is in [argument-shapes.md](argument-shapes.md), together with the `.clone()` that rustc
suggests for it and must not get.

```rust
#[derive(Default, Debug)]
struct Cfg {
    name: String,
    retries: u32,
}

#[derive(Default)]
struct CfgBuilder {
    inner: Cfg,
}

impl CfgBuilder {
    fn name(&mut self, value: &str) -> &mut Self {
        self.inner.name = value.into();
        self
    }

    fn retries(&mut self, value: u32) -> &mut Self {
        self.inner.retries = value;
        self
    }

    // Owned terminal. It needs a bound builder, never a temporary.
    fn build(self) -> Cfg {
        self.inner
    }
}

fn main() {
    let mut builder = CfgBuilder::default();
    builder.name("a").retries(3);
    for attempt in 0..3u32 {
        builder.retries(attempt);
    }
    let cfg = builder.build();
    println!("{cfg:?}");
}
```

An owned setter also supports loop-driven configuration:

```rust,run
#[derive(Default)]
struct Builder { retries: u32 }
impl Builder {
    fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
    fn build(self) -> u32 { self.retries }
}
fn main() {
    let mut builder = Builder::default();
    for retry in 1..=3 { builder = builder.with_retries(retry); }
    assert_eq!(builder.build(), 3);
}
```

Rule: choose the receiver from the call sites. Both forms support loops and conditions;
owned setters require reassignment. Reach
for `fn build(&mut self) -> T` with `std::mem::take` only when the chain must stay one expression
that starts from a temporary. That form costs two things. A second `build()` succeeds and returns
the default: measured on rustc 1.97.0, one `name("real")` setter then two builds prints
`first : Cfg { name: "real", retries: 0 }` and `second: Cfg { name: "", retries: 0 }`. It also
forces `Default` on the built type. Keep `build(self)` when a second build from the same
builder must be impossible; either setter form can use that consuming terminal method.

---

## `#[derive(Clone)]` on resource-backed types

**Severity: WARNING**

A derived `Clone` on a struct that holds a resource-backed type — `Arc<Mutex<Connection>>`,
`Arc<Pool>`, `Arc<Cache>` — does not duplicate the resource. It clones the handle. Callers
often expect an isolated copy.

Document the sharing on the type:

```rust
pub struct Pool;

/// Cloning shares the underlying connection pool.
#[derive(Clone)]
pub struct Client {
    pool: std::sync::Arc<Pool>,
}
```

For a raw handle such as `OwnedFd`, `TcpStream`, or a memory-mapped file, `Clone` does not
compile, and the compiler catches the mistake. The silent case is always `Arc<T>`.

---

## `parking_lot` and `tokio` mutexes do not poison on panic

**Severity: WARNING**

`std::sync::Mutex` poisons itself when the thread that holds the guard panics. A later
`lock()` then returns `Err(PoisonError)`. Both `parking_lot::Mutex` and `tokio::sync::Mutex`
remove poisoning: the lock is simply released on panic. Code that migrates from `std` may
still assume that poisoning protects it.

Rule: when you use a `parking_lot` or `tokio` mutex, do not rely on poison detection. If a
panicking writer can leave the guarded data inconsistent, either validate the invariant on the
reader side, or stay on `std::sync::Mutex` and handle `PoisonError` deliberately.

On `std::sync::Mutex`, state the policy at each `lock()`. This probe shows both halves: the panic
under the guard poisons the lock, and a counter whose value stays valid recovers the guard.

```rust,run
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

fn main() {
    let total = Arc::new(Mutex::new(0u32));
    let worker = Arc::clone(&total);
    let joined = thread::spawn(move || {
        let _guard = worker.lock().expect("total mutex poisoned");
        panic!("worker fails while it holds the guard");
    })
    .join();
    assert!(joined.is_err());
    // The panic poisoned the mutex. A bare `unwrap` here would panic too.
    assert!(total.lock().is_err());
    // Policy for this counter: the value stays valid, so recover the guard.
    let value = *total.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(value, 0);
}
```

Size is not speed. `parking_lot` 0.12.5 locks are smaller than or the same size as `std::sync`
locks (measured on rustc 1.98.1, aarch64-apple-darwin: `Mutex<()>` is 1 byte against 16). Only a
contention measurement justifies the switch.

---

## A bare `Condvar::wait` misses an early notify

**Severity: CRITICAL**

A `Condvar` keeps no count. A `notify_one` that arrives before the waiter calls `wait` is lost,
and the bare `wait` then blocks for ever. `wait_while` checks the predicate under the lock before
it sleeps and after every wake, so an early notify cannot hang it.

Measured on Rust 1.99.1 with a worker that notifies 200 ms before the waiter starts: the bare
`wait` hangs and `timeout 3` kills it with exit 124; `wait_while` exits 0.

```rust
use std::sync::{Condvar, Mutex};

fn wait_for_item(queue: &Mutex<u32>, ready: &Condvar) {
    let guard = ready
        .wait_while(queue.lock().expect("queue mutex poisoned"), |n| *n == 0)
        .expect("queue mutex poisoned");
    drop(guard);
}
```

Rule: wait only through `wait_while` or `wait_timeout_while`, with a predicate that reads only
state the same mutex protects.

---

## Integer overflow panics in debug and wraps in release

**Severity: WARNING**

Rust panics on integer overflow in a debug build, and wraps silently in a release build. The
usual victims are counter increments, byte-length calculations, and index arithmetic in
parsers.

```rust
let total: u32 = a + b; // panics in debug on overflow; wraps in release
```

Pick one of three replacements. For a fallible path:

```rust
let total = a.checked_add(b).ok_or(Error::Overflow)?;
```

For intentional wrapping (ring buffers, sequence numbers):

```rust
let total = a.wrapping_add(b);
```

For saturation (rate limiters, dimension clamps):

```rust
let total = a.saturating_add(b);
```

Rule: use `checked_*` on any value derived from untrusted input, and map the `None` to a
typed error. Enable `clippy::arithmetic_side_effects` on every parser and on every path that
computes a size from external data.

---

## `Arc` reference cycles without `Weak` leak permanently

**Severity: WARNING**

Reference counting cannot break a cycle. Two `Arc`s that point at each other are never
deallocated. Nothing panics and nothing errors. The process simply grows.

```rust
use std::sync::Arc;

// CYCLE: pool -> connection -> pool. Neither value is ever dropped.
struct Pool { connections: Vec<Arc<Connection>> }
struct Connection { pool: Arc<Pool> }
```

```rust
use std::sync::{Arc, Weak};

// FIX: Weak for the child-to-parent direction. The count never holds the cycle.
struct Pool { connections: Vec<Arc<Connection>> }
struct Connection { pool: Weak<Pool> }
```

Rule: in any parent-child relationship where the child needs a reference back to the parent,
use `Weak<T>` for the child-to-parent direction.

---

## A `Weak` registry never reaps dead slots

**Severity: WARNING**

`Weak` breaks the cycle, and it also moves the registration lifetime out of the registry. A
registry that stores `Vec<Weak<dyn Observer>>` accepts a registration whose only `Arc` the caller
drops on the next line. `attach` returns successfully, `notify` then delivers nothing, and no error
appears anywhere. The dead slot also stays in the `Vec` for the life of the subject, so the
registry grows without bound.

Measured on rustc 1.97.0 with two `attach` calls, one observer dropped at once: `slots=2 live=1`,
one delivery, and the `Vec` still two entries long until a reap runs.

```rust
use std::sync::{Arc, Weak};

trait Observer {
    fn observe(&self, state: u32);
}

struct Subject {
    observers: Vec<Weak<dyn Observer>>,
    state: u32,
}

impl Subject {
    fn attach(&mut self, observer: &Arc<dyn Observer>) {
        self.observers.push(Arc::downgrade(observer));
    }

    fn notify(&mut self) {
        // Reap first, or the Vec grows for the life of the subject.
        self.observers.retain(|slot| slot.strong_count() > 0);
        let state = self.state;
        for observer in self.observers.iter().filter_map(Weak::upgrade) {
            observer.observe(state);
        }
    }
}
```

Rule: reap on every notify with `retain(|slot| slot.strong_count() > 0)`. Reaping forces
`notify(&mut self)`; when notify must take `&self`, move the reap into `attach` or give the
registry interior mutability. Either the registry owns the `Arc` and hands the caller a
subscription guard, or `attach` documents that the caller owns the registration lifetime. Do not
put an associated type on the observer trait: a bare `dyn Observer` is then `error[E0191]: the
value of the associated type 'Subject' in 'Observer' must be specified`, which locks the registry
to one subject type. Take the subject as a plain method argument.

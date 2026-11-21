# Drop and RAII

Read this file before you add `impl Drop` to a type, and when a diff moves a `Drop` impl from one
type to another. `impl Drop` is not a hook you attach to a struct. It is a property of the type
that turns on seven error codes at once, at sites far from the impl.

Every result below is from rustc 1.97.0, edition 2024, `aarch64-apple-darwin`.

Contents:

- The cost table
- The four escape hatches
- Drop order
- The design rule: one guard field, never the aggregate
- Keep blocking and fallible cleanup explicit
- Leaking is safe, so a guard is never a soundness argument

This file owns the `Drop` design rule, the escape hatches, and drop order. The
`rust-compiler-errors` skill, when it is installed, owns the error-to-fix entry for each code in
the cost table, whose `Fix` column covers each code without it.

---

## The cost table

Each row is an error that `impl Drop` turns on. None of these errors exists before you add the
impl. The restriction is on the *type*, not on the body: an empty `fn drop(&mut self) {}` costs
exactly as much as a full one.

| Code | Site the error lands on | Exact message | Fix |
| --- | --- | --- | --- |
| E0509 | a move of any non-`Copy` field: `take(value.name)` | `cannot move out of type 'S', which implements the 'Drop' trait` | move `Drop` to a one-field guard |
| E0509 | destructuring: `let S { a, b } = value;` | same message, `note: move occurs because these variables have types that don't implement the 'Copy' trait` | bind by reference, or destructure a guard-free aggregate |
| E0509 | functional record update: `S { a: new, ..old }` | same message, `help: clone the value from the field instead of using the functional record update syntax` | build the whole struct explicitly |
| E0507 | `if let Some(x) = self.field` inside `Drop::drop` | `cannot move out of 'self.h' as enum variant 'Some' which is behind a mutable reference` | match on `self`, or call `self.h.take()` |
| E0184 | `#[derive(Copy)]` on the same type | `the trait 'Copy' cannot be implemented for this type; the type has a destructor` | remove `Copy`, or remove `Drop` |
| E0367 | `impl<T: Debug> Drop for S<T>` over `struct S<T>` | `'Drop' impl requires 'T: Debug' but the struct it is implemented for does not` | repeat the bound on the struct, or drop it from the impl |
| E0502, E0505 | any use of the borrowed value after the guard is created | `mutable borrow might be used here, when '_g' is dropped and runs the 'Drop' code for type 'Guard'` | close the scope, or call `drop(guard)` |
| E0597 | the borrowed value is declared after the holder | `'s' does not live long enough` plus `borrow might be used here, when '_h' is dropped and runs the 'Drop' code for type 'Holder'` | declare the borrowed value first |

`Copy` fields are exempt from E0509. `eat_u32(value.n)` with `n: u32` compiles on a `Drop` type.

Do not reach for `#[may_dangle]` to relax dropck: it is unstable (E0658) on stable Rust. Change
the declaration order instead.

---

## The four escape hatches

Pick by the field type, not by habit.

| Hatch | Requires | Leaves behind | Use it when |
| --- | --- | --- | --- |
| `self.field.take()` | the field is `Option<T>` | `None` | the field is already optional for a domain reason |
| `std::mem::take(&mut self.field)` | `T: Default` | `T::default()` | `String`, `Vec<T>`, any collection |
| `std::mem::replace(&mut self.field, cheap)` | a cheap valid `T` | `cheap` | `T` has no `Default` but a null-ish value exists |
| `ManuallyDrop::take(&mut self.field)` | `unsafe`, exactly one call | nothing valid | the field must have no empty state at all |

`mem::take` on a trait object does not compile. `Box<dyn Trait>` has no `Default`:

```text
error[E0277]: the trait bound `dyn Data: Default` is not satisfied
    = note: required for `Box<dyn Data>` to implement `Default`
note: required by a bound in `std::mem::take`
```

Measure before you reach for the `unsafe` `ManuallyDrop` form. It buys size only when the payload
has no niche. Measured with `struct BoxedHandle(Box<u32>)`: `BoxedHandle`, `Option<BoxedHandle>`
and `#[repr(transparent)] struct Guard(ManuallyDrop<BoxedHandle>)` are all 8 bytes, so the unsafe
rewrite saves nothing. With `struct PlainHandle(u32)`: 4 bytes against 8 for `Option<PlainHandle>`,
and only then does the trade have a payoff. Any `Box`, `NonNull`, `&T` or `NonZero*` payload has a
niche.

---

## Drop order

```rust
struct Noisy(&'static str);
impl Drop for Noisy {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

struct Outer {
    first: Noisy,
    second: Noisy,
    third: Noisy,
}

impl Drop for Outer {
    fn drop(&mut self) {
        println!("drop Outer body");
    }
}

fn main() {
    // prints: drop Outer body, drop f1, drop f2, drop f3
    let _outer = Outer { first: Noisy("f1"), second: Noisy("f2"), third: Noisy("f3") };
}
```

| Container | Order | Measured output |
| --- | --- | --- |
| a type with a user `Drop` | body first, then fields in **declaration** order | `drop Outer body`, `f1`, `f2`, `f3` |
| locals in a scope | **reverse** declaration order | `l3`, `l2`, `l1` |
| tuple, array | front to back | `t1`, `t2`, `t3` |
| `Vec<T>` | unspecified by the API | front to back on the measured toolchain only |
| a partially moved struct | the moved field drops where it moved to; the rest drop at end of scope | `eaten inside fn`, `drop px`, `after eat`, `drop py` |

The [Vec contract](https://doc.rust-lang.org/1.100.0/std/vec/struct.Vec.html#guarantees)
does not guarantee element drop order. When order matters, consume the elements explicitly:

```rust,run
use std::cell::RefCell;
use std::rc::Rc;

struct Record(u8, Rc<RefCell<Vec<u8>>>);
impl Drop for Record {
    fn drop(&mut self) { self.1.borrow_mut().push(self.0); }
}

fn main() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let values: Vec<_> = (0..3).map(|id| Record(id, Rc::clone(&log))).collect();
    for value in values { drop(value); }
    assert_eq!(*log.borrow(), [0, 1, 2]);
}
```

This probe checks explicit front-to-back cleanup on normal completion. If an element's
cleanup panics, it does not prove the cleanup order of the remaining elements.

The field you declare **last** stays alive longest inside a struct. This is the opposite of C++,
where members are destroyed in reverse declaration order. When one field must outlive another
during cleanup, declare the longer-lived one last, and write the reason next to it.

---

## The design rule: one guard field, never the aggregate

E0509 belongs to the type that implements `Drop`. Confine that type to one field and you confine
every cost in the table above to that field. The aggregate then keeps partial moves, and no other
field needs an `Option` or an `unwrap`.

```rust
trait Data {
    fn inspect(&self);
}

struct ErrorHandle;
impl ErrorHandle {
    fn report(&self, _message: &str) {}
}

struct NotUsedGuard(Option<ErrorHandle>);

impl NotUsedGuard {
    fn new(handle: ErrorHandle) -> Self {
        Self(Some(handle))
    }

    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for NotUsedGuard {
    fn drop(&mut self) {
        let Self(Some(handle)) = self else { return };
        handle.report("destroyed before use");
    }
}

struct Gadget {
    data: Box<dyn Data>,
    not_used: NotUsedGuard,
}

impl Gadget {
    // No Option, no unwrap: `data` is a plain field.
    fn inspect(&self) {
        self.data.inspect();
    }

    // Legal partial move: the guard field is consumed, then `data` moves out.
    fn use_it(self) -> Box<dyn Data> {
        self.not_used.disarm();
        self.data
    }
}
```

Put `impl Drop for Gadget` on the aggregate instead and `self.data` in `use_it` becomes E0509.

```rust,compile_fail,E0509
struct Sink {
    data: Vec<u8>,
}
impl Drop for Sink {
    fn drop(&mut self) {}
}

fn finish(sink: Sink) -> Vec<u8> {
    sink.data // E0509: cannot move out of type `Sink`, which implements the `Drop` trait
}
```

The reflexive repair is `data: Option<Box<dyn Data>>` plus an `unwrap` in every method, which
moves a compile-time guarantee into a run-time panic. The guard above holds an `Option` only
because `disarm` must empty it; a guard whose cleanup only reads its field holds the field plain.

A consuming method on a `Drop` type is E0509 too: `fn into_data(self) -> Vec<u8> { self.data }`
fails like `finish` above. Take the field with an escape hatch instead:
`fn into_data(mut self) -> Vec<u8> { std::mem::take(&mut self.data) }`. `drop` then runs on the
emptied value.

---

## Keep blocking and fallible cleanup explicit

`Drop::drop` returns `()`, so a cleanup error has no return path. A panic in `drop` during
unwinding aborts the process. Put fallible or blocking cleanup (a flush, a final write, a
network goodbye, a `join`) in a consuming `close(self) -> Result<T, E>` method.

Inspect the complete fallback path, including field destructors. An arbitrary `W: Write`
can block or panic in `flush()`, even when its returned `Result` is ignored. Its own destructor
can also block or panic. `BufWriter` tries to write its buffer during drop and ignores errors;
that is not a nonblocking or panic-free contract. Document that limitation when using it.
For an async owner, complete blocking cleanup off the executor thread before dropping it.

This wrapper flushes only through explicit `close`. It adds no drop-time flush or retry.
The probe checks success, an I/O error, and a panic, plus ordinary drop and outer unwinding.
Its test writer has a nonblocking, nonpanicking destructor; a generic writer needs a separate audit.

```rust,run
use std::cell::Cell;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

pub struct Log<W: Write> { out: W }
impl<W: Write> Log<W> {
    /// Flushes and returns the writer.
    ///
    /// # Errors
    /// Returns the writer's flush error.
    ///
    /// # Panics
    /// Propagates a panic from the writer's flush implementation.
    pub fn close(mut self) -> io::Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }
}

struct Probe {
    counts: Rc<(Cell<usize>, Cell<usize>)>,
    outcome: u8,
}
impl Write for Probe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> {
        self.counts.0.set(self.counts.0.get() + 1);
        match self.outcome {
            0 => Ok(()),
            1 => Err(io::Error::other("flush failed")),
            _ => panic!("flush panicked"),
        }
    }
}
impl Drop for Probe {
    fn drop(&mut self) { self.counts.1.set(self.counts.1.get() + 1); }
}

fn main() {
    for outcome in 0..5 {
        let counts = Rc::new((Cell::new(0), Cell::new(0)));
        let log = Log { out: Probe { counts: Rc::clone(&counts), outcome } };
        match outcome {
            0 => drop(log.close().ok().expect("flush succeeds")),
            1 => assert!(log.close().is_err()),
            2 => assert!(catch_unwind(AssertUnwindSafe(|| { let _ = log.close(); })).is_err()),
            3 => drop(log),
            _ => assert!(catch_unwind(AssertUnwindSafe(|| {
                let _log = log;
                panic!("outer failure");
            })).is_err()),
        }
        assert_eq!(counts.0.get(), usize::from(outcome < 3));
        assert_eq!(counts.1.get(), 1);
    }
}
```

Run the probe with unwinding enabled. It checks wrapper behavior, not the behavior of every
`Write` implementation. Under `panic = "abort"`, a panic skips these cleanup assertions.

---

## Leaking is safe, so a guard is never a soundness argument

`std::mem::forget`, `Box::leak`, a `Rc`/`Arc` cycle, `ManuallyDrop`, and `std::process::exit` all
skip the destructor. None of them is `unsafe`. Measured: `mem::forget(guard)` returns and the
`Drop` body never runs.

Destructors are guaranteed on exactly one non-local path — unwinding. A `catch_unwind` around a
scope that holds a guard still runs the guard's `drop`.

The consequence for review: **never write a safety comment whose argument is "the guard runs".**
A caller can leak the guard and every invariant the guard restores stays broken. If the invariant
is a memory-safety invariant, the design is unsound whatever the guard does. The
`rust-unsafe` skill owns the API shapes that survive a forgotten guard: make the leak harmless, as
`thread::scope` does. The `rust-pin-projection` skill owns the one guarantee the language gives:
a value behind `Pin` is dropped before its memory is reused.

Use a guard for the things leaking cannot corrupt: closing a descriptor, releasing a lock,
decrementing a metric, flushing a buffer. On Rust 1.100+, `core::mem::DropGuard`
runs a cleanup closure. The `rust-panic-safety` skill, when installed, owns rollback
and `DropGuard::dismiss`; use a local `Drop` type on an older MSRV.

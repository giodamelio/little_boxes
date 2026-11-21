# Pinning proofs, projection macros, and review checklist

Deep material for `rust-pin-projection`. `SKILL.md` holds the triage table, the verifier, the
rules, and the four structural pinning obligations. This file holds the proofs behind them, the
macro details, and the review checklist. The examples use the `SelfRef` type that
[The SelfRef example](#the-selfref-example) defines, and the `Wrapper` type that
[A hand-written Unpin impl](#a-hand-written-unpin-impl) defines.

Measured on rustc 1.99.1, edition 2024, with pin-project 1.1.13 and pin-project-lite 0.2.17. The
Miri output comes from nightly 2026-05-15 with the default Stacked Borrows model. The allocation
id depends on the program and the Miri version; the shape does not.

Contents:

- [The SelfRef example](#the-selfref-example)
- [Project two fields by hand](#project-two-fields-by-hand)
- [A hand-written Unpin impl](#a-hand-written-unpin-impl)
- [Moves that Pin does not stop](#moves-that-pin-does-not-stop)
- [How long a pin! value lives](#how-long-a-pin-value-lives)
- [pin! of a &mut reference](#pin-of-a-mut-reference)
- [A named stack binding under Miri](#a-named-stack-binding-under-miri)
- [A hand-rolled pinning macro](#a-hand-rolled-pinning-macro)
- [Choose the crate](#choose-the-crate)
- [Project in Future::poll](#project-in-futurepoll)
- [Prove the Unpin result with a test](#prove-the-unpin-result-with-a-test)
- [A destructor that moves a pinned field](#a-destructor-that-moves-a-pinned-field)
- [PinnedDrop with pin-project-lite](#pinneddrop-with-pin-project-lite)
- [PinnedDrop with pin-project](#pinneddrop-with-pin-project)
- [Pinned destruction without a macro](#pinned-destruction-without-a-macro)
- [repr(packed): only one crate rejects it](#reprpacked-only-one-crate-rejects-it)
- [Assign into a pinned value](#assign-into-a-pinned-value)
- [A pinning pointer that pins nothing](#a-pinning-pointer-that-pins-nothing)
- [Review checklist](#review-checklist)

## The SelfRef example

`SelfRef` is address-sensitive: its `ptr` field points into its own `buf` field. `init` writes
the self-pointer only after the value is pinned. The other examples in this file use this type.

```rust,run
use std::marker::PhantomPinned;
use std::pin::{pin, Pin};

/// Address-sensitive: `ptr` points into `buf` in the same value.
struct SelfRef { buf: [u8; 4], ptr: *const u8, _p: PhantomPinned }

impl SelfRef {
    fn new(v: u8) -> Self { SelfRef { buf: [v; 4], ptr: std::ptr::null(), _p: PhantomPinned } }

    /// Write the self-pointer only after the value is pinned.
    fn init(self: Pin<&mut Self>) {
        // SAFETY: no field is moved out; only `ptr` is written.
        let this = unsafe { self.get_unchecked_mut() };
        this.ptr = this.buf.as_ptr();
    }

    /// # Safety
    /// Call only after `init`, and only while the value has not moved.
    unsafe fn read(&self) -> u8 { unsafe { *self.ptr } }
}

fn main() {
    let mut on_stack = pin!(SelfRef::new(0xAA));      // Pin<&mut SelfRef>, no allocation
    on_stack.as_mut().init();
    let mut on_heap = Box::pin(SelfRef::new(0xBB));   // Pin<Box<SelfRef>>, outlives the frame
    on_heap.as_mut().init();
    assert_eq!(unsafe { (on_stack.read(), on_heap.read()) }, (0xAA, 0xBB));
}
```

## Project two fields by hand

A pinning projection returns `Pin<&mut Field>` through `Pin::map_unchecked_mut`. A field that is
not structurally pinned projects to a plain `&mut` through `Pin::get_unchecked_mut`. State the
choice for each field in the `SAFETY:` comment:

```rust,run
use std::pin::Pin;

pub struct Pair<A, B> { first: A, second: B }

impl<A, B> Pair<A, B> {
    pub fn first(self: Pin<&mut Self>) -> Pin<&mut A> {
        // SAFETY: `first` is structurally pinned. This type has no manual `Drop`,
        // no manual `Unpin` impl, is not `#[repr(packed)]`, and no method of it
        // moves out of `first`.
        unsafe { self.map_unchecked_mut(|s| &mut s.first) }
    }

    pub fn second(self: Pin<&mut Self>) -> &mut B {
        // SAFETY: `second` is not structurally pinned, so `&mut` to it is free.
        // No projection of this type returns `Pin<&mut B>`.
        unsafe { &mut self.get_unchecked_mut().second }
    }
}

fn main() {
    let mut p = std::pin::pin!(Pair { first: 1u32, second: String::new() });
    p.as_mut().second().push('x');
    assert_eq!(*p.as_mut().first(), 1);
}
```

## A hand-written Unpin impl

`Wrapper` declares `inner` structurally pinned, and one safe line then breaks the claim:

```rust
use std::pin::Pin;

struct Wrapper<T> { inner: T }

impl<T> Wrapper<T> {
    fn inner(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: claims `inner` is structurally pinned. The impl below breaks the claim.
        unsafe { self.map_unchecked_mut(|w| &mut w.inner) }
    }
}

impl<T> Unpin for Wrapper<T> {}   // one safe line, and the program is unsound
```

## Moves that Pin does not stop

`Pin::new` and `Pin::get_mut` are both safe for an `Unpin` target, and `Pin<Ptr>` implements
`DerefMut` for one. Safe code moves the value out through any of them:

```rust,run
use std::mem;
use std::pin::Pin;

fn main() {
    let (mut a, mut b) = (String::from("a"), String::from("b"));
    // Both calls are safe, because String: Unpin.
    mem::swap(Pin::new(&mut a).get_mut(), Pin::new(&mut b).get_mut());
    assert_eq!((a.as_str(), b.as_str()), ("b", "a"));

    // `Pin<Ptr>` implements `DerefMut` for an `Unpin` target, so this is safe too.
    let mut c = 0u32;
    *Pin::new(&mut c) = 7;
    assert_eq!(c, 7);
}
```

`!Unpin` does not stop a move either. Every Rust type moves, `!Unpin` included, until the value
is pinned. This program has no `unsafe`, no error, and no warning:

```rust,run
use std::marker::PhantomPinned;
use std::mem;

struct NotUnpin { x: u32, _p: PhantomPinned }

fn main() {
    let a = NotUnpin { x: 1, _p: PhantomPinned };
    let mut c = a;                                   // move by assignment
    let mut d = NotUnpin { x: 2, _p: PhantomPinned };
    mem::swap(&mut c, &mut d);                       // swap through &mut
    let boxed = Box::new(c);                         // move into the heap
    let v = vec![d];                                 // move into a Vec
    assert_eq!((boxed.x, v[0].x), (2, 1));
}
```

## How long a pin! value lives

`pin!(v)` moves `v` into a temporary. The temporary lives to the end of the enclosing block when
`pin!(..)` is the `let` initializer. It also lives that long inside a tuple, an array, a braced
struct literal, a `&`, a block tail, or an `if`, `else`, or `match` arm of that initializer. Inside
a tuple-struct or variant constructor such as `Some(..)`, it lives that long only since Rust 1.89.
As a function argument or a method receiver, it dies at the end of the statement:

- `let p = id(pin!(5u32)); println!("{}", *p);` gives `error[E0716]: temporary value dropped
  while borrowed`.
- `fn escape() -> Pin<&'static mut u32> { pin!(5u32) }` gives `error[E0515]: cannot return value
  referencing temporary value`.

Bind first (`let mut p = pin!(v);`), then pass `p.as_mut()` down. Use `Box::pin` when the pinned
value must outlive the function.

## pin! of a &mut reference

Since Rust 1.97, `pin!(r)` with `r: &mut T` always gives `Pin<&mut &mut T>`. This call fails:

```rust,compile_fail,E0308
use std::marker::PhantomPinned;
use std::pin::{pin, Pin};

struct S { n: u32, _p: PhantomPinned }

fn takes(p: Pin<&mut S>) -> u32 { p.n }

fn by_ref(s: &mut S) -> u32 {
    takes(pin!(s))   // E0308 since 1.97: `pin!(s)` is `Pin<&mut &mut S>`
}
```

The error says ``expected `S`, found `&mut S` ``. On 1.97 and 1.98 its help line also names
`unreachable_pin_macro_type_constraint`; 1.99 prints no help line. Change the parameter to `Pin<&mut S>`, or take `s: S` by
value and write `takes(pin!(s))`. Never use `unsafe { Pin::new_unchecked(s) }` here, because the
caller can still move `*s` after the call returns.

Rustc 1.88 to 1.96 sometimes coerced `pin!(r)` to `Pin<&mut T>`, and so pinned a value that the
caller could still move. Those versions also accept `pin!(&mut s)`, after which safe code moves the
local once the pin drops.

## A named stack binding under Miri

This function builds with no diagnostic:

```rust
use std::pin::Pin;

fn make() -> SelfRef {
    let mut s = SelfRef::new(0xAA);
    unsafe { Pin::new_unchecked(&mut s) }.init();   // establishes the self-pointer
    s     // moving it out compiles with zero warnings, and is undefined behaviour
}
```

Cargo, rustc, and Clippy accept it. Miri reports the read through the self-pointer after the
move. The line numbers refer to the probe, where `SelfRef` lives in a library crate:

```text
$ cargo +nightly miri run --bin stackescape
error: Undefined Behavior: memory access failed: alloc182 has been freed, so this pointer is dangling
help: alloc182 was allocated here:
 5 |     let mut s = SelfRef::new(0xAA);
help: alloc182 was deallocated here:
 8 | }
```

## A hand-rolled pinning macro

Tutorials show a two-line `macro_rules! pin` that shadows the caller's binding. As they give it,
the macro does not compile at a safe call site:

```text
error[E0133]: call to unsafe function `Pin::<Ptr>::new_unchecked` is unsafe and requires unsafe block
   |         let mut $name = Pin::new_unchecked(&mut $name);
   |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ call to unsafe function
   |     pin_hand!(a);
   |     ------------ in this macro invocation
```

The reflex fix is worse than the defect, because it compiles. Wrapping the **invocation** in
`unsafe { ... }` scopes the shadowing `let` to that block. The shadow dies at the closing brace,
the original binding comes back, and the value moves out:

```rust,run
use std::pin::Pin;

struct S { n: u32 }

macro_rules! pin_hand {
    ($name:ident) => {
        let mut $name = Pin::new_unchecked(&mut $name);
    };
}

fn takes_by_value(s: S) -> u32 { s.n }

fn main() {
    let mut a = S { n: 1 };
    unsafe { pin_hand!(a); }        // the shadow dies at this brace
    let n = takes_by_value(a);      // `a` is still an `S`, and still movable
    assert_eq!(n, 1);
}
```

That program compiles with two warnings (`unused_mut` and `unused_variables`) and runs. The pin
guarantee is gone. Use `std::pin::pin!`: it keeps the `unsafe` inside the macro body around the
call only, and it binds a temporary that no name reaches again.

## Choose the crate

| | `pin-project` | `pin-project-lite` |
| --- | --- | --- |
| Kind | proc macro, `#[pin_project]` | `macro_rules!`, `pin_project! { ... }` |
| Dependency tree | `syn`, `quote`, `proc-macro2` | none |
| Structs with named fields | yes | yes |
| Tuple structs and tuple variants | yes | no |
| Enums | yes, with `#[pin_project(project = Name)]` | yes, with `#[project = Name]` |
| Named projection type | `#[pin_project(project = Name)]` | `#[project = Name]` |
| Pinned destructor | `#[pin_project(PinnedDrop)]` plus `#[pinned_drop]` | `impl PinnedDrop` inside the macro body |
| Replace and take the old value | `#[pin_project(project_replace = Name)]` | `#[project_replace = Name]` |
| Explicit `!Unpin` marker | `#[pin_project(!Unpin)]` | `#[project(!Unpin)]`, or a `#[pin] PhantomPinned` field |
| Conditional `Unpin` of your own | `UnsafeUnpin` | no |
| `#[repr(packed)]` | rejected outright, dedicated message | `E0793` only when a field needs alignment above 1 |
| Error messages on bad input | readable | not useful; the crate docs suggest retrying the input with `pin-project` |

Take `pin-project-lite` when the dependency graph builds no proc-macro crate yet. The crate's
own documentation calls that its only advantage, and says it gives no benefit when proc-macro
dependencies are already in the graph. Take `pin-project` for a tuple struct, a tuple variant,
`UnsafeUnpin`, or the packed-struct check.

Both crates emit an `Unpin` impl conditioned on the `#[pin]` fields alone. That impl is more
permissive than the auto-derived one, which is the trap that
[Prove the Unpin result with a test](#prove-the-unpin-result-with-a-test) shows.

## Project in Future::poll

`project()` hands out `Pin<&mut F>` for a `#[pin]` field and a plain `&mut` for every other
field:

```rust
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pin_project! {
    /// Counts how many times the inner future is polled.
    pub struct Counted<F> {
        #[pin]
        inner: F,      // structurally pinned: `Future::poll` needs `Pin<&mut F>`
        polls: u32,    // not pinned: `&mut u32` is enough
    }
}

impl<F: Future> Future for Counted<F> {
    type Output = (F::Output, u32);
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();      // Pin<&mut F> for `inner`, &mut u32 for `polls`
        *this.polls += 1;
        match this.inner.poll(cx) {
            Poll::Ready(v) => Poll::Ready((v, *this.polls)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn main() {
    let mut fut = Box::pin(Counted { inner: std::future::ready(7u8), polls: 0 });
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready((7, 1)));
}
```

## Prove the Unpin result with a test

A forgotten `#[pin]` attribute is silent. `fn assert_unpin<T: Unpin>() {}` states the result in
one line, and the direction of the test decides where it goes.

A type that you promise is `Unpin` takes a normal unit test:

```rust
use pin_project_lite::pin_project;

pin_project! { pub struct Buf { #[pin] inner: Vec<u8> } }

fn assert_unpin<T: Unpin>() {}

fn main() { assert_unpin::<Buf>(); }   // fails to compile if `inner` becomes !Unpin
```

A type that must stay `!Unpin` needs the opposite: the assertion has to fail. Put it in a
`compile_fail` doctest or a `trybuild` case, never in a normal test. A `compile_fail` doctest
passes on any compile error, so check the error text once by hand, or use `trybuild`, which
compares it on every run:

```rust,compile_fail,E0277
use pin_project_lite::pin_project;
use std::marker::PhantomPinned;

pin_project! { struct Marked { #[pin] _p: PhantomPinned } }

fn assert_unpin<T: Unpin>() {}

fn main() { assert_unpin::<Marked>(); }
```

The failure text names the generated origin type, which is how you tell a macro-derived `Unpin`
from the auto one:

```text
error[E0277]: `PhantomPinned` cannot be unpinned
  | fn main() { assert_unpin::<Marked>(); }
  |                            ^^^^^^ within `__Origin<'_>`, the trait `Unpin` is not implemented for `PhantomPinned`
  = note: consider using the `pin!` macro
          consider using `Box::pin` if you need to access the pinned value outside of the current scope
note: required because it appears within the type `__Origin<'_>`
note: required for `Marked` to implement `Unpin`
  = note: this error originates in the macro `$crate::__pin_project_make_unpin_impl` which comes from the expansion of the macro `pin_project`
```

A projection macro makes the forgotten attribute silent. Both macros replace the auto `Unpin`
impl with one that reads the `#[pin]` fields alone, so this compiles, and the struct is `Unpin`:

```rust
use pin_project_lite::pin_project;
use std::marker::PhantomPinned;

pin_project! { struct Forgot { _p: PhantomPinned } }   // no #[pin]

fn assert_unpin<T: Unpin>() {}

fn main() {
    assert_unpin::<Forgot>();   // the struct is Unpin: the attribute was forgotten
}
```

## A destructor that moves a pinned field

`fn drop(&mut self)` receives `&mut self` even for a value that was pinned, as if the compiler
called `Pin::get_unchecked_mut` for you. A destructor can therefore move a structurally pinned
field out in plain safe code:

```rust
use std::cell::Cell;
use std::mem;

// `Holder` projects `inner` as structurally pinned, as `Wrapper` does above.
struct Holder { inner: SelfRef }

thread_local! { static STOLEN: Cell<Option<SelfRef>> = const { Cell::new(None) }; }

impl Drop for Holder {
    fn drop(&mut self) {
        // Safe code that moves a structurally pinned field out of pinned storage.
        STOLEN.set(Some(mem::replace(&mut self.inner, SelfRef::new(0))));
    }
}
```

Miri (`cargo +nightly miri run --bin dropmove`) reports `memory access failed: allocN has been
freed, so this pointer is dangling` at the next read of the self-pointer. Use `PinnedDrop`, or the
inner-function pattern below.

## PinnedDrop with pin-project-lite

Put the impl inside the macro body. The receiver is `Pin<&mut Self>`, so no field can move out:

```rust
use pin_project_lite::pin_project;
use std::pin::Pin;

pin_project! {
    pub struct Conn<S> {
        #[pin]
        stream: S,
        id: u32,
    }
    impl<S> PinnedDrop for Conn<S> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            println!("closing {}", this.id);
        }
    }
}

fn main() {
    drop(Box::pin(Conn { stream: (), id: 7 }));   // prints: closing 7
}
```

## PinnedDrop with pin-project

Pass `PinnedDrop` to the attribute, then write a separate `#[pinned_drop]` impl:

```rust
use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;

#[pin_project(PinnedDrop)]
pub struct Conn<S> {
    #[pin]
    stream: S,
    id: u32,
}

#[pinned_drop]
impl<S> PinnedDrop for Conn<S> {
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();
        println!("closing {}", this.id);
    }
}
```

A plain `impl Drop` beside either macro is a compile error. The message names a generated marker
trait, and the name differs per crate:

```text
$ cargo build --bin ppdrop            # pin-project
error[E0119]: conflicting implementations of trait `SMustNotImplDrop` for type `S<_>`

$ cargo build --bin plitedrop         # pin-project-lite
error[E0119]: conflicting implementations of trait `MustNotImplDrop` for type `S<_>`
```

## Pinned destruction without a macro

For a hand-written address-sensitive type, put the real destructor body in an inner function that
takes the pin. The signature then stops you from moving a field out:

```rust,run
use std::pin::Pin;

struct Conn { id: u32 }

impl Drop for Conn {
    fn drop(&mut self) {
        // SAFETY: the value is never used again after `drop` returns.
        inner_drop(unsafe { Pin::new_unchecked(self) });

        fn inner_drop(this: Pin<&mut Conn>) {
            // The real destructor body. It cannot move a field out.
            assert!(this.id > 0);
        }
    }
}

fn main() { drop(Conn { id: 1 }); }
```

## repr(packed): only one crate rejects it

`#[pin_project]` carries a dedicated check. It fires on every packed struct:

```text
error: #[pin_project] attribute may not be used on #[repr(packed)] types
4 | #[repr(packed)]
  |        ^^^^^^
```

`pin_project_lite::pin_project!` has no such check. It emits a
`#[forbid(unaligned_references, safe_packed_borrows)] fn __assert_not_repr_packed` helper that takes
a reference to every field. The error fires only when a field needs alignment above 1:

```text
error[E0793]: reference to field of packed struct is unaligned
3 | / pin_project! {
4 | |     #[repr(packed)]
5 | |     struct S { #[pin] p: PhantomPinned, n: u64 }
6 | | }
  = note: this struct is 1-byte aligned, but the type of this field may require higher alignment
  = note: this error originates in the macro `$crate::__pin_project_struct_make_proj_method` which comes from the expansion of the macro `pin_project`
```

Change `n: u64` to `n: u8`, and every field is 1-byte aligned. The check stays quiet, the struct
compiles, and `.project()` hands out `Pin<&mut PhantomPinned>` for the packed field. Do not treat
`pin-project-lite` as a guard against `#[repr(packed)]`. Reject packed layouts yourself.

Neither check reaches a hand-written projection. `unsafe { Pin::new_unchecked(&mut packed) }`
compiles with no diagnostic at all.

## Assign into a pinned value

`Pin::<Ptr>::set` needs only `Ptr: DerefMut`, with no `Unpin` bound. It is safe because it drops
the old value in place before it writes the new one, which keeps the drop guarantee:

```rust,run
use std::marker::PhantomPinned;
use std::pin::pin;

struct S { id: u32, _p: PhantomPinned }

impl Drop for S {
    fn drop(&mut self) { println!("drop {}", self.id); }
}

fn main() {
    let mut p = pin!(S { id: 1, _p: PhantomPinned });
    p.set(S { id: 2, _p: PhantomPinned });   // safe: drops the old S in place first
    assert_eq!(p.id, 2);
}
```

The program prints `drop 1`, then `drop 2`. Use `Pin::set` instead of
`unsafe { get_unchecked_mut() }` plus an assignment.

## A pinning pointer that pins nothing

`Pin<P>` promises a stable address for `P::Target`. The promise is empty when `P` stores the
target inline, because moving the `Pin<P>` value moves the target too:

```rust
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

struct Inline<T>(T);                        // not a pointer: T lives inline
impl<T> Deref for Inline<T> { type Target = T; fn deref(&self) -> &T { &self.0 } }
impl<T> DerefMut for Inline<T> { fn deref_mut(&mut self) -> &mut T { &mut self.0 } }

fn build() -> Pin<Inline<SelfRef>> {
    let mut p = unsafe { Pin::new_unchecked(Inline(SelfRef::new(0xAA))) };
    p.as_mut().init();
    p            // moves the "pinned" SelfRef, and that is undefined behaviour
}
```

The line numbers refer to the probe, where `SelfRef` lives in a library crate:

```text
$ cargo +nightly miri run --bin derefinline
error: Undefined Behavior: memory access failed: alloc187 has been freed, so this pointer is dangling
12 |     pub unsafe fn read(&self) -> u8 { unsafe { *self.ptr } }
   |                                                ^^^^^^^^^ Undefined Behavior occurred here
help: alloc187 was allocated here:
10 |     let mut p = unsafe { Pin::new_unchecked(Inline(SelfRef::new(0xAA))) };
help: alloc187 was deallocated here:
13 | }
```

The only move in that program is `p` leaving `build`. `Box<T>`, `Rc<T>`, and `Arc<T>` are sound
pinning pointers because the target lives in an allocation that a move of the pointer does not
touch.

For a pointer type that a caller may wrap in `Pin`:

- `Deref::Target` must live behind a real indirection, at an address that survives a move of the
  pointer.
- `Deref` and `DerefMut` must not move out of the pointee, and must not invalidate it.
- `Drop` must not move out of the pointee, and must run its destructor before it frees storage.

## Review checklist

Use this list when you review code that pins or projects. Each item restates a rule from
`SKILL.md` or from this file:

- Every `Pin` in a signature sits on a `!Unpin` type, or a trait forces it.
- Every address-sensitive type is `!Unpin`: a `PhantomPinned` field (with `#[pin]` inside a
  projection macro), or a `!Unpin` attribute on the macro.
- No `Pin::new_unchecked` call outside `Drop::drop` takes `&mut` to a nameable binding, and no
  hand-written pinning macro exists in the tree.
- No `pin!(&mut x)`, or `pin!(r)` with `r: &mut T`, stands in for pinning the value itself.
- No hand-written `impl Unpin` sits next to a pinning projection.
- No manual `impl Drop` sits on a type with a structurally pinned field, unless it moves its body
  into an inner function that takes `Pin<&mut Self>`. Under a projection macro, `PinnedDrop` does.
- No pinned type is `#[repr(packed)]`.
- No projection exposes `take`, `replace`, or `swap` on a structurally pinned field.
- Every `map_unchecked_mut` and `get_unchecked_mut` call carries a `SAFETY:` comment that names
  the field and the choice.
- No pointer type that a caller may wrap in `Pin` stores its target inline.

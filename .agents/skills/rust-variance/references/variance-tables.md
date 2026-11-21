# Variance tables, probe files, and worked repairs

Deep material for `rust-variance`. Every result below comes from rustc 1.98.1, edition 2024,
aarch64-apple-darwin, and Miri 0.1.0 on nightly 2026-05-15.

Contents:

- The probe files: covariance, contravariance, and the note for each invariant type
- Probe traps: the `'static` outer reference and the per-version unsize table
- What each `PhantomData` form declares
- The unsound contravariant channel
- Worked pairs, rejected and repaired: the invariant `Sender`, a trait bound of your own, a free
  lifetime in `dyn Trait`, a public type that turns invariant, and a `&mut` parameter
- Raw pointers: the exact-owner accessor and the trait-object cast

## The probe files

Compile with a real output path. Never pass `rustc -o /dev/null`: rustc renames its temporary
output onto that path. As root, the rename replaces the `/dev/null` device with a regular file
and exits 0. As a normal user, it fails with a temp-directory error even when the code is
correct:

```bash
rustc --edition 2024 --crate-type lib --emit=metadata probe.rs -o probe.rmeta
```

### Covariance: every one of these compiles

```rust
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

struct Own<T>(PhantomData<T>);

fn p01<'a, 'b: 'a>(x: &'b u8) -> &'a u8 { x }
fn p02<'a, 'b: 'a>(x: Box<&'b u8>) -> Box<&'a u8> { x }
fn p03<'a, 'b: 'a>(x: Vec<&'b u8>) -> Vec<&'a u8> { x }
fn p04<'a, 'b: 'a>(x: *const &'b u8) -> *const &'a u8 { x }
fn p05<'a, 'b: 'a>(x: fn() -> &'b u8) -> fn() -> &'a u8 { x }
fn p06<'a, 'b: 'a>(x: PhantomData<&'b u8>) -> PhantomData<&'a u8> { x }
fn p07<'a, 'b: 'a>(x: PhantomData<fn() -> &'b u8>) -> PhantomData<fn() -> &'a u8> { x }
fn p08<'a, 'b: 'a>(x: PhantomData<*const &'b u8>) -> PhantomData<*const &'a u8> { x }
fn p09<'a, 'b: 'a>(x: PhantomData<&'b mut u8>) -> PhantomData<&'a mut u8> { x }
fn p10<'a, 'b: 'a>(x: Vec<Box<dyn Debug + 'b>>) -> Vec<Box<dyn Debug + 'a>> { x }
fn p11<'a, 'b: 'a>(x: &'b mut u8) -> &'a mut u8 { x }
fn p12<'a, 'b: 'a>(x: Own<&'b u8>) -> Own<&'a u8> { x }
fn p13<'a, 'b: 'a>(x: Rc<&'b u8>) -> Rc<&'a u8> { x }
fn p14<'a, 'b: 'a>(x: Arc<&'b u8>) -> Arc<&'a u8> { x }
fn p15<'a, 'b: 'a>(x: Option<&'b u8>) -> Option<&'a u8> { x }
fn p16<'a, 'b: 'a>(x: Result<&'b u8, &'b str>) -> Result<&'a u8, &'a str> { x }
fn p17<'a, 'b: 'a>(x: (&'b u8, u32)) -> (&'a u8, u32) { x }
fn p18<'a, 'b: 'a>(x: [&'b u8; 3]) -> [&'a u8; 3] { x }
fn p19<'a, 'b: 'a>(x: HashMap<u32, &'b u8>) -> HashMap<u32, &'a u8> { x }
fn p20<'x, 'a, 'b: 'a>(x: &'x [&'b u8]) -> &'x [&'a u8] where 'b: 'x { x }
```

Note `p11`: `&'a mut T` is covariant in `'a`, the lifetime of the borrow itself. It is invariant
in `T`. The two facts sit in one type and are easy to confuse.

### Contravariance: only these two compile

```rust
use std::marker::PhantomData;

fn c1<'a, 'b: 'a>(x: fn(&'a u8)) -> fn(&'b u8) { x }
fn c2<'a, 'b: 'a>(x: PhantomData<fn(&'a u8)>) -> PhantomData<fn(&'b u8)> { x }
```

`&'a u8`, `fn() -> &'a u8`, and `Box<dyn Fn(&'a u8)>` are all rejected in the contravariant
direction with `error: lifetime may not live long enough`.

### Invariance: the exact note for each row

Each line below is one probe of the shape `fn p<'a, 'b: 'a>(x: C<&'b u8>) -> C<&'a u8> { x }`,
followed by the note rustc prints. All of them fail.

| Probe type | rustc note |
| --- | --- |
| `Cell<&'b u8>` | the struct `Cell<T>` is invariant over the parameter `T` |
| `UnsafeCell<&'b u8>` | the struct `UnsafeCell<T>` is invariant over the parameter `T` |
| `RefCell<&'b u8>` | the struct `RefCell<T>` is invariant over the parameter `T` |
| `Mutex<&'b u8>` | the struct `std::sync::Mutex<T>` is invariant over the parameter `T` |
| `RwLock<&'b u8>` | the struct `std::sync::RwLock<T>` is invariant over the parameter `T` |
| `mpsc::Sender<&'b u8>` | the struct `std::sync::mpsc::Sender<T>` is invariant over the parameter `T` |
| `*mut &'b u8` | mutable pointers are invariant over their type parameter |
| `&'x mut &'b u8` | mutable references are invariant over their type parameter |
| `PhantomData<Cell<&'b u8>>` | the struct `Cell<T>` is invariant over the parameter `T` |
| `PhantomData<*mut &'b u8>` | mutable pointers are invariant over their type parameter |
| `fn(&'b u8) -> &'b u8` | no variance note |
| `Box<dyn Fn(&'b u8)>` | no variance note |
| `Box<dyn Fn() -> &'b u8>` | no variance note |
| `mpsc::Receiver<&'b u8>` | the struct `std::sync::mpsc::Receiver<T>` is invariant over the parameter `T` |
| `mpsc::SyncSender<&'b u8>` | the struct `SyncSender<T>` is invariant over the parameter `T` |
| `AtomicPtr<&'b u8>` | the struct `Atomic<T>` is invariant over the parameter `T` |

`AtomicPtr<T>` is an alias of `Atomic<*mut T>` in 1.98.1, so a script that greps for `AtomicPtr`
misses its note.

A parameter used both covariantly and contravariantly is invariant (Reference, "Variance"). rustc
prints ``the struct `Mixed<T>` is invariant over the parameter `T` `` for this probe:

```rust,compile_fail
pub struct Mixed<T> { pub get: fn() -> T, pub put: fn(T) }

fn cov<'a, 'b: 'a>(x: Mixed<&'b u8>) -> Mixed<&'a u8> { x }
```

Every row, the three `fn` and `dyn` rows included, prints
`help: consider adding the following bound: 'a: 'b` under the error. Only a row that prints a
variance note also prints
`help: see <https://doc.rust-lang.org/nomicon/subtyping.html> for more information about variance`.
The three `fn` and `dyn` rows print no variance note, so grep for the error text alone when you
script this. The type path inside a note follows the probe's imports: a glob import such as
`use std::cell::*;` prints `std::cell::Cell<T>`. Match on `is invariant over the parameter`, not
on the path.

## Probe traps

**Never put `'static` in the outer position.** This compiles and proves nothing. The outer
`&'static mut` forces `&'b u8: 'static`, which collapses `'a` and `'b` to the same lifetime:

```rust
fn degenerate<'a, 'b: 'a>(x: &'static mut &'b u8) -> &'static mut &'a u8 { x }
```

The correct form uses a fresh outer lifetime and explicit outlives bounds. It is rejected, which
is the true answer:

```rust,compile_fail
fn honest<'x, 'a: 'x, 'b: 'a>(x: &'x mut &'b u8) -> &'x mut &'a u8 { x }
```

**A probe on a type parameter needs a lifetime inside it.** `C<u8>` cannot show variance,
because `u8` has no subtypes. Always probe with `&'b u8`.

**An unsize step turns the probe into a coercion test.** Each row is one probe
`fn p<'a, 'b: 'a>(x: IN) -> OUT { x }`. Every row holds an invariant `Cell`, `Mutex`, or `*mut`;
a row that compiles does so through an unsize coercion:

| `IN` | `OUT` | 1.97.1 | 1.98.1 |
| --- | --- | --- | --- |
| `Cell<&'b u8>` | `Cell<&'a u8>` | rejected | rejected |
| `Cell<&'b [u8]>` | `Cell<&'a [u8]>` | rejected | rejected |
| `Cell<&'b u8>` | `Cell<&'a dyn Send>` | compiles | compiles |
| `Cell<Box<dyn Debug + 'b>>` | `Cell<Box<dyn Debug + 'a>>` | compiles | compiles |
| `Cell<&'b mut [u8; 3]>` | `Cell<&'a mut [u8]>` | rejected | compiles |
| `Cell<&'b mut i32>` | `Cell<&'a mut dyn Send>` | rejected | compiles |
| `Cell<&'b mut (dyn Send + 'b)>` | `Cell<&'a mut (dyn Send + 'a)>` | rejected | compiles |
| `*mut (dyn Debug + 'b)` | `*mut (dyn Debug + 'a)` | compiles | compiles |
| `Vec<Cell<Box<dyn Debug + 'b>>>` | `Vec<Cell<Box<dyn Debug + 'a>>>` | rejected | rejected |
| `Mutex<&'b (dyn Debug + 'b)>` | `Mutex<&'a (dyn Debug + 'a)>` | rejected | rejected |

The 1.98 rows come from rust-lang/rust#149219, which lets a `&mut` unsize coercion shorten its
lifetime inside an invariant wrapper. Rust 1.88.0 gives the 1.97.1 result on every row. `RefCell` and `UnsafeCell` behave like `Cell` here.
`Mutex`, `RwLock`, and `Vec` do not forward unsize coercions, so the `Mutex` and `Vec` rows stay
rejected. Wrap a probe type in `Vec<...>` to keep the coercion out of the verdict. The CI
toolchain type-checks the two rows that surprise most:

```rust
use std::cell::Cell;
use std::fmt::Debug;

fn via_dyn<'a, 'b: 'a>(x: Cell<Box<dyn Debug + 'b>>) -> Cell<Box<dyn Debug + 'a>> { x }
fn via_mut_unsize<'a, 'b: 'a>(x: Cell<&'b mut i32>) -> Cell<&'a mut dyn Send> { x }
```

## What each `PhantomData` form declares

| Form | Variance in `T` | `Send` / `Sync` | Use it for |
| --- | --- | --- | --- |
| `PhantomData<T>` | covariant | both follow `T` | the struct owns a `T` it does not store |
| `PhantomData<&'a T>` | covariant | both need `T: Sync` | the struct borrows a `T` for `'a` |
| `PhantomData<&'a mut T>` | invariant in `T`, covariant in `'a` | `Send` needs `T: Send`, `Sync` needs `T: Sync` | an exclusive borrow |
| `PhantomData<fn() -> T>` | covariant | always both, whatever `T` is | a type-state tag that is never built |
| `PhantomData<fn(T)>` | contravariant | always both, whatever `T` is | a consumer that never stores a `T` |
| `PhantomData<fn(T) -> T>` | invariant | always both, whatever `T` is | an invariant tag that must stay `Send` and `Sync` |
| `PhantomData<*const T>` | covariant | neither, whatever `T` is | a raw handle that must stay on one thread |
| `PhantomData<*mut T>` | invariant | neither, whatever `T` is | a raw handle through which `T` is written, so it must also be invariant |
| `PhantomData<Cell<T>>` | invariant | `Send` needs `T: Send`, never `Sync` | interior mutability held behind a pointer |

`PhantomData<T>` also tells drop check that the struct owns a `T`. The other forms do not. On
stable (measured on Rust 1.99.1 and 1.98.1) the difference shows in one case only: the struct has
no `Drop` impl of its own, but another field gives it drop glue. Then a `T` with a borrow and a
`Drop` impl must outlive the struct:

```rust,compile_fail,E0597
use std::marker::PhantomData;

struct Loud<'a>(&'a str);
impl Drop for Loud<'_> { fn drop(&mut self) { assert!(!self.0.is_empty()) } }

// No `Drop` impl, but the `String` field gives `Owns` drop glue.
struct Owns<T> { _name: String, _owns: PhantomData<T> }

fn owns<'a>(_: &'a str) -> Owns<Loud<'a>> { Owns { _name: String::new(), _owns: PhantomData } }

fn main() {
    let _o;
    let s = String::from("x");
    _o = owns(&s); // `PhantomData<fn() -> T>` here compiles
}
```

The marker has no drop-check effect in the other stable cases. A plain `impl Drop` already
forces every type parameter to strictly outlive the value. A struct with no drop glue is never
dropped. The marker carries the most weight next to `unsafe impl<#[may_dangle] T> Drop`, which
needs nightly and `#![feature(dropck_eyepatch)]`; `Vec<T>` and `Box<T>` use it.

The `rust-unsafe` skill, when it is installed, covers the same markers from the FFI side, and
the `rust-discipline` skill covers `PhantomData<fn() -> S>` on type-state APIs.

## The unsound contravariant channel

A handle whose only mention of `T` is `PhantomData<fn(T)>` is contravariant in `T`. That lets
`Sender<Message<'short>>` be used as `Sender<Message<'static>>`, so the handle escapes into a
`'static` context while the shared buffer still holds `Message<'short>` values. When the last
handle drops, the buffer runs their destructors, after the borrowed data is gone.

```rust
// UB: the queue drops `Message<'short>` values after `message` is freed.
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

struct Slot { data: *mut u8, drop_fn: unsafe fn(*mut u8) }

unsafe fn drop_as<T>(p: *mut u8) { drop(unsafe { Box::from_raw(p as *mut T) }) }

struct Queue { slots: RefCell<Vec<Slot>> }

impl Drop for Queue {
    fn drop(&mut self) {
        for s in self.slots.borrow_mut().drain(..) { unsafe { (s.drop_fn)(s.data) } }
    }
}

// The only mention of T is `fn(T)`, so Sender<T> is contravariant in T.
struct Sender<T> { q: Rc<Queue>, _m: PhantomData<fn(T)> }

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self { Sender { q: self.q.clone(), _m: PhantomData } }
}

impl<T> Sender<T> {
    fn send(&self, v: T) {
        let data = Box::into_raw(Box::new(v)) as *mut u8;
        self.q.slots.borrow_mut().push(Slot { data, drop_fn: drop_as::<T> });
    }
}

struct Message<'a>(&'a str);

impl Drop for Message<'_> {
    fn drop(&mut self) { println!("dropping message: {}", self.0); }
}

fn stash(s: Sender<Message<'static>>) -> Box<dyn FnOnce()> {
    Box::new(move || { let _keep = s; })
}

fn main() {
    let escaped;
    {
        let message = String::from("world");
        let q = Rc::new(Queue { slots: RefCell::new(Vec::new()) });
        let tx: Sender<Message<'_>> = Sender { q, _m: PhantomData };
        escaped = stash(tx.clone());          // contravariance widens 'short to 'static
        tx.send(Message(&message));
    }
    println!("block over");
    drop(escaped);
}
```

The program compiles with no warning and no `unsafe` at the call sites that matter:

```text
$ cargo run
block over
dropping message: <freed bytes>
$ cargo +nightly miri run
error: Undefined Behavior: constructing invalid value of type &str: encountered a dangling reference (use-after-free)
     = note: stack backtrace (excerpt):
             <Message<'_> as std::ops::Drop>::drop
             drop_as::<Message<'_>>
             <Queue as std::ops::Drop>::drop
```

Replace `Queue::drop` with a version that leaks the slots instead of calling `drop_fn`, and Miri
reports a leak and no undefined behavior. That isolates the destructor as the whole cause.

Rules that follow:

- Contravariance is sound for a handle that consumes a `T` inside the call and never stores or
  drops one. A queue stores and drops, so its handle must stay invariant.
- `std::sync::mpsc::Sender<T>` is invariant for this reason, not by oversight.
- A `PhantomData<fn(T)>` on a type that owns a `T` is a soundness bug, not a style choice.
  Use `PhantomData<T>`.

## The invariant `Sender`, rejected and repaired

`std::sync::mpsc::Sender<T>` is invariant in `T`, so one call site that demands `'static` pins the
whole channel. This pair is rejected:

```rust,compile_fail,E0597
use std::sync::mpsc::{channel, Sender};

struct Message<'a> { description: &'a str }

fn short_lived<'a>(_storage: &'a String, _s: Sender<Message<'a>>) {}
fn long_lived(_s: Sender<Message<'static>>) {}

fn main() {
    let storage = "descr".to_string();
    let (tx, _rx) = channel();
    short_lived(&storage, tx.clone());
    long_lived(tx.clone());
}
```

```text
error[E0597]: `storage` does not live long enough
11 |     short_lived(&storage, tx.clone());
   |                 ^^^^^^^^ borrowed value does not live long enough
12 |     long_lived(tx.clone());
   |     ---------------------- argument requires that `storage` is borrowed for `'static`
```

The message is covariant even though the handle is not. Write consumers generic over the
lifetime, and coerce at the send site:

```rust
use std::sync::mpsc::{channel, Sender};

struct Message<'a> { description: &'a str }

fn any_lifetime(_s: Sender<Message<'_>>) {}                     // not Message<'static>
fn send_static<'a>(s: &Sender<Message<'a>>, m: Message<'static>) { let _ = s.send(m); }

fn main() {
    let storage = "descr".to_string();
    let (tx, _rx) = channel();
    any_lifetime(tx.clone());
    send_static(&tx, Message { description: "literal" });
    let _ = tx.send(Message { description: &storage });
}
```

## A trait bound of your own matches by equality

The `note:` points at your own bound. `S: Sink<&'static u8>` is not `S: Sink<&'a u8>`:

```rust,compile_fail,E0597
trait Sink<T> { fn put(&self, v: T); }

fn feed<'a, S: Sink<&'a u8>>(s: &S, v: &'a u8) { s.put(v) }

fn go<S: Sink<&'static u8>>(s: &S) {
    let local = 5u8;
    feed(s, &local);   // E0597: argument requires that `local` is borrowed for `'static`
}
```

## A free lifetime in `dyn Trait`, both forms

A callback field as a function pointer is contravariant in the event lifetime:

```rust
pub struct Event<'a> { pub name: &'a str }
pub struct HooksPtr<'a> { pub on: fn(&Event<'a>) }

fn lengthen<'a>(h: HooksPtr<'a>) -> HooksPtr<'static> { h }   // compiles
```

Swap the field for a boxed closure over the same lifetime and the struct becomes invariant.
Both directions now fail:

```rust,compile_fail
pub struct Event<'a> { pub name: &'a str }
pub struct HooksBox<'a> { pub on: Box<dyn Fn(&Event<'a>)> }

fn lengthen<'a>(h: HooksBox<'a>) -> HooksBox<'static> { h }
```

```text
error: lifetime may not live long enough
4 | fn lengthen<'a>(h: HooksBox<'a>) -> HooksBox<'static> { h }
  |             -- lifetime `'a` defined here               ^ returning this value requires that `'a` must outlive `'static`
  = note: the struct `HooksBox<'a>` is invariant over the parameter `'a`
```

## A public type that turns invariant

The first version shortens its lifetime:

```rust
pub struct ConfigV1<'a> { pub name: &'a str }

fn shorten<'a>(c: ConfigV1<'static>, _tie: &'a str) -> ConfigV1<'a> { c }
```

A `Cell` field breaks the same function for every downstream caller:

```rust,compile_fail
use std::cell::Cell;

pub struct ConfigV2<'a> { pub name: Cell<&'a str> }

fn shorten<'a>(c: ConfigV2<'static>, _tie: &'a str) -> ConfigV2<'a> { c }
```

```text
error: lifetime may not live long enough
5 | fn shorten<'a>(c: ConfigV2<'static>, _tie: &'a str) -> ConfigV2<'a> { c }
  |            -- lifetime `'a` defined here                              ^ returning this value requires that `'a` must outlive `'static`
  = note: the struct `ConfigV2<'a>` is invariant over the parameter `'a`
```

## A `&mut` parameter, rejected and repaired

The caller's type annotation propagates into the argument through the invariant `&mut`:

```rust,compile_fail,E0597
fn push_str_ref<'a>(v: &mut Vec<&'a str>, s: &'a str) { v.push(s) }

fn main() {
    let mut v: Vec<&'static str> = vec!["a"];
    let s = String::from("b");
    push_str_ref(&mut v, &s);
}
```

```text
error[E0597]: `s` does not live long enough
4 |     let mut v: Vec<&'static str> = vec!["a"];
  |                ----------------- type annotation requires that `s` is borrowed for `'static`
6 |     push_str_ref(&mut v, &s);
  |                          ^^ borrowed value does not live long enough
```

Take the container by value and give it back. The `&mut` disappears, so covariance applies and a
`Vec<&'static str>` coerces at the call:

```rust
fn with_str_ref<'a>(mut v: Vec<&'a str>, s: &'a str) -> Vec<&'a str> { v.push(s); v }

fn main() {
    let v: Vec<&'static str> = vec!["a"];
    let s = String::from("b");
    let v = with_str_ref(v, &s);
    assert_eq!(v.len(), 2);
}
```

## Raw pointers: the exact-owner accessor and the trait-object cast

This exact-owner form ties the output to the owner and needs no raw pointer dereference:

```rust,run
fn deref_tied<'a, T>(owner: &'a T, pointer: *const T) -> Option<&'a T> {
    std::ptr::eq(owner, pointer).then_some(owner)
}

fn main() {
    let owner = String::from("live");
    let pointer = &owner as *const String;
    assert_eq!(deref_tied(&owner, pointer).map(String::as_str), Some("live"));

    let other = String::from("other");
    assert!(deref_tied(&owner, &other).is_none());
}
```

For a pointer into a larger allocation, make the wrapper borrow the allocation owner or return a
guard that holds it.

Since Rust 1.94 a raw-pointer cast cannot extend the lifetime bound of a trait object. Older
unsafe code that erases a bound this way fails with
`note: raw pointer casts of trait objects cannot extend lifetimes`:

```rust,compile_fail
pub trait Tr {}

fn erase<'a>(p: *mut (dyn Tr + 'a)) -> *mut (dyn Tr + 'static) { p as _ }
```

The longer bound can make a method with a `where Self: 'x` bound callable through a vtable that
has no entry for it (rust-lang/rust#141402). Prefer to keep `'a` in the stored type. Use
`transmute::<*mut (dyn Tr + 'a), *mut (dyn Tr + 'static)>` only when no method of `Tr` has a
lifetime bound on `Self` and the owner outlives every use of the pointer. State both facts in
the `SAFETY` comment.

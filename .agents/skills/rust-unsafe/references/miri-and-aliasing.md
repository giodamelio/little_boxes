# Aliasing, auto traits, and Miri

The proof side of unsafe: what makes an abstraction unsound after it compiles, and the checks
that find it. [unsafe-patterns.md](unsafe-patterns.md) holds the patterns you write.
[SKILL.md](../SKILL.md) holds the rules.

Contents:

- Manual `unsafe impl Send` or `Sync`: the checklist, per-field assertions, the derive leak
- Reference fabrication with `RefCell::as_ptr`
- The `ManuallyDrop<String>` fabrication, and why Miri clears it
- Aliasing models: Stacked Borrows and Tree Borrows
- Miri invocations, and FFI code that Miri cannot run
- Clippy invocations for unsafe

Miri messages below were measured on Miri nightly-2026-11-09 unless a line says otherwise. The
numbers in `<N>` and `allocN` change from run to run.

## Manual `unsafe impl Send` or `Sync`

The compiler accepts a manual `unsafe impl Send` or `Sync` without a check, also after a field
gains an `Rc<_>`; a data race or a double free follows at run time. Often a field-type change
removes the need for the impl; the `rust-send-sync` skill, when it is installed, shows how.
Before you write the impl:

1. List every field type. Confirm that each is `Send` or `Sync`, or state why the wrapper keeps
   the invariant despite the field.
2. Check every `&self` method, derives included. A derived `Debug`, `Clone`, `PartialEq`, or
   `Hash` hands `&T` to `T`'s own impl on whichever thread calls it.
3. Assert the auto trait on each field type, not on the wrapper, because the manual impl is
   unconditional. `clippy::non_send_fields_in_send_ty` (nursery) flags a non-`Send` field under
   a manual `Send` impl.
4. Write a `// SAFETY:` comment on the impl that names the fields audited and the argument.

Objects from a C or C++ library are usually not thread-safe, even when the Rust binding
compiles.

### Asserting an auto trait on each field

A manual `unsafe impl Send` on a wrapper is unconditional, so it stays accepted after the fields
change. Assert the fields, not the wrapper. The assertion needs no dependency:

```rust
pub struct Inner {
    pub id: u32,
}

pub struct MyWrapper {
    pub inner: Inner,
}
unsafe impl Send for MyWrapper {}

const _: () = {
    fn assert_send<T: Send>() {}
    let _ = assert_send::<Inner>;
};
```

An `Inner` that gains an `Rc<_>` field fails this with E0277.

A negative assertion, that a handle stays `!Send`, has no clean stable form. Use
`static_assertions::assert_not_impl_all!` (last release 1.1.0, 2019-11-03) or a `compile_fail`
doctest.

`unsafe impl Send for &T` is E0321; `T: Sync` is the only lever, because `&T: Send` holds exactly
when `T: Sync`. The `rust-send-sync` skill, when it is installed, has the diagnostic.

### A derive is a `&self` API

"Callers cannot obtain `&T` from `&Wrapper<T>`, so `Wrapper<T>` is always `Sync`" is a false
safety argument, and it survives review because the leak is generated code. `#[derive(Debug)]`,
`#[derive(Clone)]`, `#[derive(PartialEq)]`, `#[derive(Hash)]`, and a derived `Serialize` each
produce a `&self` method that hands `&T` to `T`'s own impl. `Wrapper<T>: Sync` means
`&Wrapper<T>: Send`, so that `&T` reaches a second thread and `T`'s impl mutates its interior
state there with no synchronization.

```rust
#[derive(Debug)]                              // <- this derive is the leak
pub struct NoSharedAccess<T>(T);
impl<T> NoSharedAccess<T> {
    pub fn get_mut(&mut self) -> &mut T { &mut self.0 }   // only `&mut`, looks safe
}
unsafe impl<T> Sync for NoSharedAccess<T> {}  // UNSOUND
```

Give the wrapper a `T` whose `Debug` impl clones an `Rc`, and the non-atomic refcount races. Two
scoped threads and 300 iterations under Miri are enough:

```text
error: Undefined Behavior: Data race detected between (1) non-atomic write on thread `unnamed-1`
       and (2) non-atomic read on thread `unnamed-2` at alloc270
```

The access kinds in the message change with the interleaving; the `Data race detected` prefix
does not.

An unconditional `unsafe impl<T> Sync for Wrapper<T> {}` is sound only for a type with no `&self`
API at all, derives included. Otherwise bound it, `unsafe impl<T: Sync> Sync for Wrapper<T> {}`,
or delete the manual impl and let the auto impl decide.

Do not decide this with a native stress test. A raced non-atomic refcount is symmetric: a lost
decrement leaks, a lost increment frees early, and the leak direction has no symptom. Four
threads by three million balanced clone-and-drop pairs through an unsound `Sync` exited 0 with a
strong count of 11427 where 1 was correct, and never crashed. Miri reports the race on a few
hundred iterations. Run it with `-Zmiri-many-seeds`, because one seed explores one thread
schedule. Use Miri.

## Reference fabrication with `RefCell::as_ptr`

`RefCell::as_ptr` returns the raw pointer and does not touch the dynamic borrow counter. An
`unsafe` deref that hands a caller a `&'a T` or a `&'a mut T` therefore produces a reference the
`RefCell` does not track. A later `borrow_mut()` succeeds instead of panicking, and safe caller
code mutates the data behind a live shared reference.

```rust
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    // Unsound: `as_ptr` skips the borrow flag, so `leaked` is not exclusive.
    let cell = Rc::new(RefCell::new(String::from("moo")));
    let leaked: &String = unsafe { &*cell.as_ptr() };
    cell.borrow_mut().push_str(" MOO"); // Safe code. No `already borrowed` panic.
    println!("{leaked}");               // Prints `moo MOO`.
}
```

Measured on rustc 1.99.1, edition 2024: the program compiles, prints `moo MOO`, and exits 0. A
`&String` observed a mutation and nothing panicked.

The pattern appears when a borrowing iterator is written over `Rc<RefCell<T>>`. The safe form
does not compile. Returning `&*cell.borrow()` from `next` is `error[E0515]: cannot return value
referencing temporary value`, because the `Ref` guard dies at the end of `next`. `as_ptr` plus
`unsafe` removes the error and leaves the API unsound.

Change the API shape. In order of preference:

1. Yield the guard: `type Item = Ref<'a, T>`. The caller holds the borrow, so the counter works.
2. Yield an owned handle, `Rc<RefCell<T>>`, and let the caller call `borrow()` itself.
3. Store the elements in a `Vec` or an arena and iterate a real slice. No interior mutability
   and no unsafe.

```rust
use std::cell::{Ref, RefCell};
use std::rc::Rc;

pub struct List<T> { items: Vec<Rc<RefCell<T>>> }
pub struct Iter<'a, T> { inner: std::slice::Iter<'a, Rc<RefCell<T>>> }

impl<T> List<T> {
    pub fn iter(&self) -> Iter<'_, T> { Iter { inner: self.items.iter() } }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Ref<'a, T>;
    fn next(&mut self) -> Option<Ref<'a, T>> { Some(self.inner.next()?.borrow()) }
}
```

Both Miri aliasing models reject the unsound form, but only when the program interleaves the
fabricated reference with a mutation. A test suite that never holds a yielded reference across a
`borrow_mut()` passes Miri clean. Treat the pattern as UB on inspection. Miri is a confirmation
here, never the gate. Run it under both models, the second with
`MIRIFLAGS="-Zmiri-tree-borrows"`; the `rust-sanitizers-miri` skill, when it is installed, has
the two messages.

A reference derived from `UnsafeCell::get` must obey the same aliasing rules: no two live
`&mut`, and no mutation under a live `&T`.

## The `ManuallyDrop<String>` fabrication, and why Miri clears it

This pattern builds a `&String` out of a `&str` by rebuilding the `String` header over the
borrowed buffer and then suppressing the destructor:

```rust
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;

pub struct StringRef<'a> {
    data: ManuallyDrop<String>,
    _lifetime: PhantomData<&'a str>,
}

impl<'a> StringRef<'a> {
    pub fn new(s: &'a str) -> Self {
        // SAFETY: there is none. `String::from_raw_parts` requires a buffer that
        // came from the global allocator with exactly this capacity, and a `&str`
        // supplies neither guarantee.
        let data = unsafe {
            ManuallyDrop::new(String::from_raw_parts(s.as_ptr() as *mut u8, s.len(), s.len()))
        };
        Self { data, _lifetime: PhantomData }
    }
}

impl Deref for StringRef<'_> {
    type Target = String;
    fn deref(&self) -> &String { &self.data }
}
```

Measured on Miri nightly-2026-11-09: this runs clean under the default Stacked Borrows, under
`-Zmiri-tree-borrows`, and under `-Zmiri-strict-provenance`, for a `&'static str` literal and for
a sub-slice of a heap `String`, including a `.clone()` of the deref target. The provenance is
correct, because the pointer comes from a live allocation. The allocator and capacity
precondition is observable only at deallocation, and `ManuallyDrop` prevents that. Miri has
nothing to report.

The private field is therefore the whole safety argument, and `ManuallyDrop::into_inner` is a
safe method that removes it:

```rust
let lit = "Hello World";
let s = unsafe {
    ManuallyDrop::new(String::from_raw_parts(lit.as_ptr() as *mut u8, lit.len(), lit.len()))
};
let owned: String = ManuallyDrop::into_inner(s);   // a safe call; UB when `owned` drops
```

```text
error: Undefined Behavior: constructing invalid value of type &mut [u8]:
       encountered mutable reference pointing to read-only memory
```

A sub-slice of a heap `String` fails differently, with `trying to retag from <737> for Unique
permission ... but that tag only grants SharedReadOnly permission for this location`. Without
`ManuallyDrop` at all, the native binary aborts with SIGABRT and prints nothing. Never expose the
field, never derive `Clone`, and never add `DerefMut`. Accept `&str` or `impl AsRef<str>`
instead.

## Aliasing models: Stacked Borrows and Tree Borrows

Miri checks unsafe code against a formal aliasing model. Run the default model (Stacked Borrows)
first, then Tree Borrows (`MIRIFLAGS="-Zmiri-tree-borrows"`) as a second opinion; the `rust-sanitizers-miri` skill, when it is
installed, owns the flags.

A write through a raw pointer invalidates a shared borrow taken after the pointer. Both models
reject the later use of the shared borrow:

```rust
fn main() {
    let mut x = 5u32;
    let raw = &mut x as *mut u32;
    let shared = &x;        // a shared borrow of `x`
    unsafe { *raw = 6 };    // the write invalidates `shared`
    println!("{shared}");   // VIOLATION: `shared` is used after the write
}
```

```text
Stacked Borrows: error: Undefined Behavior: trying to retag from <502> for SharedReadOnly
                 permission at alloc179[0x0], but that tag does not exist in the borrow stack
                 for this location
Tree Borrows:    error: Undefined Behavior: reborrow through <479> at alloc179[0x0] is forbidden
```

A read through `raw` in place of the write passes both models, even when `shared` is used
afterward. Do not write a test that reads, and conclude that the pattern is sound.

Under Stacked Borrows the rules are:

1. Each borrow pushes a new tag onto the borrow stack for that location.
2. A `&mut T` access pops every borrow above it, which invalidates them.
3. A `&T` access stays valid while the shared reference is on the stack.
4. A raw-pointer access requires its tag to still be on the stack.

Tree Borrows replaces the stack with a tree and tracks each pointer's permission separately,
which is what makes it more permissive. The practical guidance is the same under both: a write
through one pointer or reference invalidates every other live pointer and reference to the same
place, except the ones it was derived from. Re-derive after the write instead of keeping an old
reference.

### Never materialize two `&mut` from one raw pointer

Both models reject it, and no compile-time diagnostic exists for it. Each `&mut *p` performs a
`Unique` retag, and the second retag kills the first reference:

```rust
let s1 = unsafe { &mut *p };   // p: *mut State
let s2 = unsafe { &mut *p };   // second Unique retag invalidates s1
s1.counter += 1;               // Stacked Borrows fails here
s2.counter += 10;              // Tree Borrows fails here
```

```text
error: Undefined Behavior: attempting a read access using <506> at alloc179[0x0],
       but that tag does not exist in the borrow stack for this location

error: Undefined Behavior: read access through <488> at alloc179[0x0] is forbidden
```

The shape hides in two common designs. One is state smuggled through `Waker::data()`, where every
concurrently polled future rebuilds a `&mut State` from the same pointer. The other is a
parameter extractor that hands out `&mut T` per parameter. Keep the `*mut T` raw, pass it down,
and form at most one `&mut` at a time from it.

### Moving a `Box` invalidates every pointer taken from it earlier

`Box<T>` wraps `Unique<T>`, so a move retags it and every outstanding tag derived from the old box
dies. Move the `Box` into its final owner **first**, then take the pointer you hand to foreign
code:

```rust
// BAD: the pointer is taken before the Box reaches its final home.
let mut b = Box::new(7u32);
let p: *mut u32 = &mut *b;
let moved = b;              // Unique retag invalidates `p`
unsafe { *p += 1; }         // UB under both models
```

```rust
// GOOD: settle the allocation first, then derive the pointer.
struct Guard { cb: Box<u32> }
let mut g = Guard { cb: Box::new(7u32) };
let p: *mut u32 = &mut *g.cb;
unsafe { *p += 1; }
```

Stacked Borrows blames the raw write; Tree Borrows blames the later use of the moved box:

```text
help: <557> was later invalidated at offsets [0x0..0x4] by a Unique retag
   |     let moved = b;                 // move the Box

error: Undefined Behavior: reborrow through <535> at alloc261[0x0] is forbidden
   = help: the accessed tag <535> has state Disabled which forbids this reborrow
```

## Miri invocations

Steps 2 to 4 of "Verify an unsafe change" in `SKILL.md`, in full:

2. Every change: `cargo test --locked -p <crate>` in the debug profile. On the executed path,
   debug assertions check many std unsafe preconditions (1.78), a misaligned `*p` (1.70), and a
   null `*p` (1.86). A violation aborts with `unsafe precondition(s) violated: ...`,
   `misaligned pointer dereference`, or `null pointer dereference occurred`: a non-unwinding
   panic that `catch_unwind` cannot catch. Treat it as undefined behavior. `--release` removes
   these checks. With the prebuilt std, no profile checks a misaligned `ptr::read` (rustc
   1.99.1); `cargo +nightly careful test`, when cargo-careful is installed, does.
3. Code that Miri can execute, when nightly with the `miri` component is installed:
   `cargo +nightly miri test --locked -p <crate>` with the default Stacked Borrows model first,
   then `MIRIFLAGS="-Zmiri-tree-borrows"` as a second opinion. Tree Borrows is more permissive
   and more experimental, so never use it to excuse a default-model failure. Add
   `-Zmiri-symbolic-alignment-check` for reads through a cast byte pointer,
   `-Zmiri-strict-provenance` for code that does not deliberately expose provenance, and
   `-Zmiri-many-seeds` for threads or a manual `Send` or `Sync` impl (one seed explores one
   schedule).
4. FFI code that Miri cannot execute: mark the test `#[cfg_attr(miri, ignore)]` and cover the
   path with ASan on the host, or HWASan or MTE on a device. cargo-careful is not a substitute.
   A `#[cfg(miri)]` stub for the foreign function must dereference every pointer that the
   foreign side stores; otherwise Miri sees no use of the pointer and reports nothing.

A clean Miri run is evidence about aliasing, provenance, initialization, and alignment (with
`-Zmiri-symbolic-alignment-check`) on the paths that the tests execute. It is not evidence about
unexecuted paths, foreign code, or a library contract that the abstract machine never evaluates,
such as UTF-8 validity or the allocator rule of `String::from_raw_parts`. The
`rust-sanitizers-miri` skill, when it is installed, owns the Miri flag policy and the sanitizers;
add a Tree Borrows lane with `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test --locked`.


```bash
cargo +nightly miri test --locked
```

The `rust-sanitizers-miri` skill, when it is installed, owns the `MIRIFLAGS` set and when to add
each flag.

Miri cannot execute a foreign function. Skip a test that crosses a real FFI boundary, and cover
that path with ASan on the host, or HWASan or MTE on a device. `cargo +nightly careful test` adds
only the std precondition checks; it is not a substitute:

```rust
#[test]
#[cfg_attr(miri, ignore)]
fn ffi_roundtrip() { /* ... */ }
```

A `#[cfg(miri)]` stub for the foreign function lets more of a crate run under Miri. Make the
stub keep and later dereference every pointer the real library stores. A stub that ignores the
pointer lets an aliasing defect pass both models; a stub that dereferences it makes both report
it. Do not count a `-Zmiri-native-lib` run as evidence for the FFI path: the Miri README calls
it experimental and unsound, because Miri stops tracking initialization and provenance on memory
shared with native code. The `rust-sanitizers-miri` skill has the stubbing strategy and the
sanitizer runs, and the `rust-test-tools` skill has `cargo-careful`, when they are installed.

## Clippy invocations for unsafe

```bash
cargo clippy --locked --all-targets -- \
  -W clippy::undocumented_unsafe_blocks \
  -W clippy::multiple_unsafe_ops_per_block \
  -W clippy::transmute_undefined_repr \
  -W clippy::ptr_as_ptr
```

Use the command line only to try a lint out. Once you keep a lint, move it into
`[workspace.lints]` so that CI and every developer get the same result, as the `rust-lints`
skill, when it is installed, describes.

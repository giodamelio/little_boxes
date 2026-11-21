# Storing callables and shaping generic parameters

Deep material for [SKILL.md](../SKILL.md). Every quoted diagnostic comes from rustc 1.98.1,
edition 2024, on aarch64-apple-darwin. The size and allocation claims are runnable probes: copy
one into a scratch binary and run it. Read only the section that matches the task.

- [Store a callable in a field](#store-a-callable-in-a-field)
- [A generic callable field cannot be named](#a-generic-callable-field-cannot-be-named)
- [Declare your own callable trait](#declare-your-own-callable-trait)
- [Arc and Rc of `dyn Fn` do not implement `Fn`](#arc-and-rc-of-dyn-fn-do-not-implement-fn)
- [Field sizes are measurable](#field-sizes-are-measurable)
- [`Box<dyn Fn>` allocates for a non-zero-sized closure value](#boxdyn-fn-allocates-for-a-non-zero-sized-closure-value)
- [By-value `impl Trait` plus a delegating impl is a monomorphization bomb](#by-value-impl-trait-plus-a-delegating-impl-is-a-monomorphization-bomb)
- [`impl Trait` in argument position is not a named generic parameter](#impl-trait-in-argument-position-is-not-a-named-generic-parameter)
- [Unstable escape hatches for a callable field](#unstable-escape-hatches-for-a-callable-field)
- [Composite outputs: RPITIT](#composite-outputs-rpitit)
- [Composite outputs: GAT against a lifetime on the trait](#composite-outputs-gat-against-a-lifetime-on-the-trait)
- [Repeat the opaque return type in an RPITIT impl](#repeat-the-opaque-return-type-in-an-rpitit-impl)

## Store a callable in a field

Four shapes, and the choice is not stylistic.

| Field type | Size | Captures | Different expressions in one `Vec` | Caller can name the type | Allocates |
| --- | --- | --- | --- | --- | --- |
| `F` on `struct S<F: Fn(u32)>` | Size of `F`. 0 for a non-capturing closure | Yes | No, `E0308` | No, `E0747` / `E0562` | No |
| `Box<dyn Fn(u32)>` | Two machine words, fat pointer | Yes | Yes | Yes | Only if the closure value is not zero-sized |
| `fn(u32)` | One machine word | **No** | Yes | Yes | No |
| `B` on `struct S<B: MyTrait>` with a blanket impl | Size of `B`. 0 for a user unit struct | Yes | No | Yes, if the user declares a named type | No |

Each column has its own section below: the sizes in
[Field sizes are measurable](#field-sizes-are-measurable), the allocation in
[`Box<dyn Fn>` allocates for a non-zero-sized closure value](#boxdyn-fn-allocates-for-a-non-zero-sized-closure-value),
the naming errors in [A generic callable field cannot be named](#a-generic-callable-field-cannot-be-named),
and the user-trait row in [Declare your own callable trait](#declare-your-own-callable-trait).

## A generic callable field cannot be named

A function item has no surface syntax. In type position the parser reads the path as a const
generic argument, so the diagnostic is about constants:

```rust,compile_fail,E0747
struct DropGuard<T, F: FnOnce(T)>(T, F);
fn report(_: u32) {}
struct Gadget { not_used: DropGuard<u32, report> }
```

```text
error[E0747]: constant provided when a type was expected
3 | struct Gadget { not_used: DropGuard<u32, report> }
  |                                          ^^^^^^
  = help: `report` is a function item, not a type
  = help: function item types cannot be named directly
```

Returning `-> Gadget<impl Fn(u32)>` only defers the problem. The value flows on into generic
positions, but the caller can never *name* it, so a field type that must be written is E0562:

```rust,compile_fail,E0562
struct Gadget<F: Fn(u32)> { f: F }
fn new_gadget() -> Gadget<impl Fn(u32)> { Gadget { f: |x| { let _ = x; } } }
struct Holder { g: Gadget<impl Fn(u32)> }
```

```text
error[E0562]: `impl Trait` is not allowed in field types
3 | struct Holder { g: Gadget<impl Fn(u32)> }
  |                           ^^^^^^^^^^^^
  = note: `impl Trait` is only allowed in arguments and return types of functions and methods
```

What the generic parameter does **not** block is `dyn`. Erasure happens at your trait, not at the
struct, so every monomorphisation gets a vtable:

```rust
struct Gadget<F: Fn(u32)> { f: F }
trait Run { fn run(&self); }
impl<F: Fn(u32)> Run for Gadget<F> { fn run(&self) { (self.f)(1) } }

fn main() {
    let a = Gadget { f: |x| assert_eq!(x, 1) };
    let b = Gadget { f: |x| assert!(x > 0) };
    // `vec![a, b]` is E0308: `a` and `b` come from different closure expressions.
    let v: Vec<Box<dyn Run>> = vec![Box::new(a), Box::new(b)];
    for g in &v { g.run(); }
}
```

## Declare your own callable trait

This is the shape for a public generic type that stores user-supplied behaviour. One method, plus
a blanket impl over `FnOnce`. Callers keep passing closures. A caller who needs a nameable,
zero-sized callable declares one:

```rust,run
pub trait DropBehavior<T> { fn on_drop(self, val: T); }

impl<T, F: FnOnce(T)> DropBehavior<T> for F {
    fn on_drop(self, val: T) { self(val) }
}

pub struct DropGuard<T, B: DropBehavior<T>> {
    val: std::mem::ManuallyDrop<T>,
    beh: std::mem::ManuallyDrop<B>,
}

// A downstream crate can name this. A closure type has no name.
pub struct ReportNotUsed;
impl DropBehavior<u32> for ReportNotUsed {
    fn on_drop(self, v: u32) { assert_eq!(v, 7); }
}

fn main() {
    assert_eq!(size_of::<DropGuard<u32, ReportNotUsed>>(), size_of::<u32>());
    assert_eq!(size_of::<DropGuard<u32, fn(u32)>>(), 2 * size_of::<usize>());
    ReportNotUsed.on_drop(7u32);
    (|v: u32| assert_eq!(v, 7)).on_drop(7u32);
}
```

Coherence accepts the second impl, here and across a crate boundary, because `ReportNotUsed` does
not satisfy `FnOnce(u32)` and rustc knows every impl a local type has. The orphan rule keeps this
true: only the crate that defines `ReportNotUsed` can implement a foreign trait such as `FnOnce` for
it. The `fn_traits` gate is not the reason; a blanket impl over `Display` behaves the same. The
payoff is in the two `size_of` lines: one `u32` against two machine words.

## Arc and Rc of `dyn Fn` do not implement `Fn`

`std` ships `impl<F: Fn + ?Sized> Fn for &F` and the same for `Box<F>`. It ships nothing for `Rc`
or `Arc`. A shared callback stored as `Arc<dyn Fn(..)>` for cheap cloning therefore satisfies no
`F: Fn(..)` bound:

```rust,compile_fail,E0277
use std::rc::Rc;
use std::sync::Arc;

fn takes<F: Fn()>(_f: F) {}

fn main() {
    let b: Box<dyn Fn()> = Box::new(|| {});
    takes(b);                             // ok
    let r: &dyn Fn() = &|| {};
    takes(r);                             // ok

    let a: Arc<dyn Fn()> = Arc::new(|| {});
    takes(a);                             // E0277
    let c: Rc<dyn Fn()> = Rc::new(|| {});
    takes(c);                             // E0277
}
```

```text
error[E0277]: expected an `Fn()` closure, found `Arc<dyn Fn()>`
13 |     takes(a);
   |     ----- ^ expected an `Fn()` closure, found `Arc<dyn Fn()>`
   = help: the trait `Fn()` is not implemented for `Arc<dyn Fn()>`
   = note: wrap the `Arc<dyn Fn()>` in a closure with no arguments: `|| { /* code */ }`
```

Two repairs. Take `Arc<dyn Fn(..)>` in the signature and call it directly, which is the honest
form for a shared callback. Or follow the `note:` and wrap: `takes(move || a())`.

## Field sizes are measurable

The field sizes in the [field table](#store-a-callable-in-a-field) above are measurable. The coercion from a `fn` item to a `fn`
pointer is silent (no warning, no lint) and adds one machine word:

```rust,run
use std::mem::{size_of, size_of_val};

fn h(_: u64) {}

fn main() {
    let f = h;
    assert_eq!(size_of_val(&f), 0);              // fn item: its own ZST
    let g: fn(u64) = h;
    assert_eq!(size_of_val(&g), size_of::<usize>()); // one machine word
    assert_eq!(size_of::<Box<dyn Fn()>>(), 2 * size_of::<usize>()); // fat pointer
    assert_eq!(size_of::<&dyn Fn()>(), 2 * size_of::<usize>());
}
```

## `Box<dyn Fn>` allocates for a non-zero-sized closure value

A non-capturing closure is a ZST. A closure that captures only a zero-sized
value can also be a ZST. `Box` of either value does not call the allocator; it
stores a dangling, well-aligned pointer. Measure the value size, not whether a
capture exists:

```rust,run
use std::alloc::{GlobalAlloc, Layout, System};
use std::mem::{size_of, size_of_val};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p, l) } }
}

#[global_allocator]
static A: Counting = Counting;

fn main() {
    let before = ALLOCS.load(Ordering::Relaxed);
    let f: Box<dyn Fn()> = Box::new(|| {});
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - before, 0);   // 0 allocations

    struct Marker;
    let marker = Marker;
    let zst_capture = move || { let _ = &marker; };
    assert_eq!(size_of_val(&zst_capture), 0);
    let mark = ALLOCS.load(Ordering::Relaxed);
    let z: Box<dyn Fn()> = Box::new(zst_capture);
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - mark, 0);     // 0 allocations

    let s = String::from("x");
    let mark = ALLOCS.load(Ordering::Relaxed);
    let g: Box<dyn Fn()> = Box::new(move || { let _ = &s; });
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - mark, 1);     // 1 allocation

    f();
    z();
    g();
    assert_eq!(size_of::<Box<dyn Fn()>>(), 2 * size_of::<usize>());
}
```

The residual cost of a boxed ZST callback is two machine words inline and one indirect call.

## By-value `impl Trait` plus a delegating impl is a monomorphization bomb

The `rust-discipline` skill recommends the delegating impl `impl<H: Handler + ?Sized> Handler for &mut H`
to stop a `&mut` field from infecting every signature with a lifetime. That recommendation is
correct. It becomes a trap the moment a trait method takes the same trait **by value** and the
body re-borrows into a recursive call: each level adds one `&mut`, and on a self-recursive data
type the chain never terminates.

`std::io::Write` already carries such a delegating impl, so this reproduces with no custom trait:

```rust,ignore
use std::io::{Result, Write};

struct Tree { tag: u8, kids: Vec<Tree> }

trait Serialize { fn ser(&self, out: impl Write) -> Result<()>; }

impl Serialize for Tree {
    fn ser(&self, mut out: impl Write) -> Result<()> {
        out.write_all(&[self.tag])?;
        for k in &self.kids { k.ser(&mut out)?; }   // &mut &mut &mut ...
        Ok(())
    }
}

fn main() {
    let t = Tree { tag: 1, kids: vec![Tree { tag: 2, kids: vec![] }] };
    t.ser(Vec::new()).unwrap();
}
```

`cargo check` on this binary exits 0. `cargo build` fails, because `main` instantiates the method:

```text
error: reached the recursion limit while instantiating `<Tree as Serialize>::ser::<&mut &mut &mut &mut &mut &mut &mut ...>`
10 |         for k in &self.kids { k.ser(&mut out)?; }
   |                               ^^^^^^^^^^^^^^^
   = note: the full name for the type has been written to '<crate>.long-type-<hash>.txt'
```

Two consequences for CI. First, a `cargo check`-only gate is blind to this class of failure, and
so is `cargo clippy`; both stop before monomorphization. A library-only `cargo build` also exits
0, because the generic method has no instantiation in the library. Add a test or example that
calls the method with a concrete writer, such as `t.ser(Vec::new())`, then run `cargo test` or
`cargo build --all-targets`. Second, when the message is
`error[E0275]: overflow evaluating the requirement ...` with a `help: consider increasing the
recursion limit`, do not raise `#![recursion_limit]`. The chain is unbounded and a higher limit
only moves the failure.

Take the writer by reference instead. Then the type is fixed at every level. `&mut dyn Write`
also stops the chain, keeps the trait dyn compatible, and costs one indirect call per write. Its
signature is `fn ser(&self, out: &mut dyn Write) -> Result<()>`. The generic form below keeps
static dispatch:

```rust
use std::io::{Result, Write};

struct Tree { tag: u8, kids: Vec<Tree> }

trait Serialize { fn ser(&self, out: &mut impl Write) -> Result<()>; }

impl Serialize for Tree {
    fn ser(&self, out: &mut impl Write) -> Result<()> {
        out.write_all(&[self.tag])?;
        for k in &self.kids { k.ser(out)?; }
        Ok(())
    }
}

fn main() {
    let t = Tree { tag: 1, kids: vec![Tree { tag: 2, kids: vec![] }] };
    t.ser(&mut Vec::new()).unwrap();
}
```

`&mut impl Write` is not a drop-in replacement, because `impl Trait` carries an implicit `Sized`
bound. A caller holding a `&mut dyn Write` is rejected:

```text
error[E0277]: the size for values of type `dyn std::io::Write` cannot be known at compilation time
9 |     a(d2);
  |     - ^^ doesn't have a size known at compile-time
note: required by an implicit `Sized` bound in `a`
2 | fn a(_: &mut impl Write) {}
  |              ^^^^^^^^^^ required by the implicit `Sized` requirement on this type parameter in `a`
help: consider relaxing the implicit `Sized` restriction
2 | fn a(_: &mut impl Write + ?Sized) {}
  |                         ++++++++
```

Write `&mut (impl Write + ?Sized)`, or `fn f<W: Write + ?Sized>(out: &mut W)`. Both accept
`&mut dyn Write`.

## `impl Trait` in argument position is not a named generic parameter

They are semantically identical and syntactically distinct in two places that break callers.

A caller cannot turbofish an `impl Trait` parameter, because the function declares zero generic
parameters:

```rust,compile_fail,E0107
use std::io::Write;
fn a(_: impl Write) {}
fn b<W: Write>(_: W) {}

fn main() {
    b::<Vec<u8>>(Vec::new());   // ok
    a::<Vec<u8>>(Vec::new());   // error
}
```

```text
error[E0107]: function takes 0 generic arguments but 1 generic argument was supplied
6 |     a::<Vec<u8>>(Vec::new());
  |     ^----------- help: remove the unnecessary generics
note: function defined here, with 0 generic parameters
  = note: `impl Trait` cannot be explicitly specified as a generic argument
```

Declare `fn f<W: Trait>(w: W)` whenever a caller may need to pin the type: inference ambiguity, a
`Default::default()` argument, or an empty collection literal.

In a trait, the impl must repeat the declaration's spelling:

```rust,compile_fail,E0643
pub trait Ser { fn ser(&self, out: impl std::io::Write); }
impl Ser for u8 { fn ser<W: std::io::Write>(&self, _out: W) {} }
```

```text
error[E0643]: method `ser` has incompatible signature for trait
1 | pub trait Ser { fn ser(&self, out: impl std::io::Write); }
  |                                    ------------------- declaration in trait here
2 | impl Ser for u8 { fn ser<W: std::io::Write>(&self, _out: W) {} }
  |                          ^ expected `impl Trait`, found generic parameter
help: try removing the generic parameter and using `impl Trait` instead
```

The mirror image, an `impl Trait` in the impl against a `<W: Trait>` declaration, is the same
E0643 with `expected generic parameter, found impl Trait`. Switching a published trait method
between the two forms breaks every implementor, even though the two signatures mean the same
thing. Pick one at publication and keep it.

Two more consequences of `impl Trait` in a trait method's argument position. It is a hidden
generic parameter, so it makes the trait not dyn compatible. A method *declaration* cannot bind
it as `mut`: `fn ser(&self, mut out: impl Write);` reports `patterns aren't allowed in functions
without bodies`. That is the deny-by-default future-incompatibility lint
`patterns_in_fns_without_body`, not a hard error, so do not `#[allow]` it. Put the `mut` on the
impl only.

## Unstable escape hatches for a callable field

`impl Trait` in an associated type still needs nightly. A stable callable field needs
a named type or a generic parameter:

```rust,compile_fail,E0658
trait DropBehavior { type OnDrop: FnOnce(u32); }
struct NotUsed;
impl DropBehavior for NotUsed {
    type OnDrop = impl FnOnce(u32);
}
```

```text
error[E0658]: `impl Trait` in associated types is unstable
4 |     type OnDrop = impl FnOnce(u32);
  |                   ^^^^^^^^^^^^^^^^
  = note: see issue #63063 for more information

error: unconstrained opaque type
  = note: `OnDrop` must be used in combination with a concrete type within the same impl
```

`std::mem::DropGuard` is stable since Rust 1.100. It accepts an `FnOnce(T)` closure,
but does not make a closure's anonymous type nameable in a field. The `rust-panic-safety`
skill, when installed, owns rollback with `DropGuard::dismiss`.
`fn_traits` still needs nightly, so no user type can implement `FnOnce` on stable. That
gate does not make the blanket-impl pattern in
[Declare your own callable trait](#declare-your-own-callable-trait) coherent; the orphan rule does.

## Composite outputs: RPITIT

The shape `SKILL.md` names. The callback becomes a named unit struct, and the method returns
`impl Ord` captured over `'a`. `use<..>` in a trait needs Rust 1.87. Keep it: in every edition, an
RPITIT without it also captures the anonymous `&mut self` lifetime, and
`key.project(&arr[j]) < key.project(&arr[i])` then fails with
`error[E0499]: cannot borrow key as mutable more than once at a time`.

```rust
use std::cmp::Reverse;
struct Order { country: String, code: u32 }

trait KeyProjection<In> {
    fn project<'a>(&mut self, i: &'a In) -> impl Ord + use<'a, Self, In> where Self: 'a;
}

struct Country;
impl KeyProjection<Order> for Country {
    // Repeat the opaque form. A concrete `Reverse<&'a String>` here warns.
    fn project<'a>(&mut self, i: &'a Order) -> impl Ord + use<'a> where Self: 'a {
        Reverse(&i.country)
    }
}

fn sort_by_key<T>(arr: &mut [T], mut key: impl KeyProjection<T>) {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if key.project(&arr[j]) < key.project(&arr[i]) { arr.swap(i, j); }
        }
    }
}

fn main() {
    let mut v = vec![
        Order { country: "at".into(), code: 1 },
        Order { country: "cz".into(), code: 3 },
    ];
    sort_by_key(&mut v, Country);
    assert_eq!(v[0].code, 3);   // Reverse => descending
}
```

## Composite outputs: GAT against a lifetime on the trait

Use a GAT when callers must name the output as an associated type. This version works with an
element type that itself borrows:

```rust
trait KeyProjection<In> {
    type Out<'a>: Ord where Self: 'a, In: 'a;
    fn project<'a>(&mut self, i: &'a In) -> Self::Out<'a>;
}

struct Order<'s> { country: &'s str, code: u32 }

struct Country;
impl<'s> KeyProjection<Order<'s>> for Country {
    type Out<'a> = std::cmp::Reverse<&'a str> where Self: 'a, Order<'s>: 'a;
    fn project<'a>(&mut self, i: &'a Order<'s>) -> Self::Out<'a> {
        std::cmp::Reverse(i.country)
    }
}

fn sort_by_key<T>(arr: &mut [T], mut key: impl KeyProjection<T>) {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if key.project(&arr[j]) < key.project(&arr[i]) { arr.swap(i, j); }
        }
    }
}

fn main() {
    let s = String::from("at");
    let mut v = vec![
        Order { country: &s, code: 1 },
        Order { country: "cz", code: 3 },
    ];
    sort_by_key(&mut v, Country);
    assert_eq!(v[0].code, 3);
}
```

Do not put the lifetime on the trait instead. `for<'a> K: KeyProjection<'a, T>` quantifies over
every `'a`, `'static` included, so it works only while no outlives bound ties the element
lifetime to `'a`. The bare impl `impl<'a, 's> KeyProjection<'a, Order<'s>>` compiles, because
`&'a Order<'s>` in the method signature already implies `'s: 'a`. Write `'s: 'a` on the impl, and
the `for<'a>` turns it into `'s: 'static` at every call site. A borrowed `Order<'s>` then fails:

```text
error[E0597]: `s1` does not live long enough
17 |     let mut v = vec![Order { country: &s1, code: 1 }, ...];
   |                                       ^^^ borrowed value does not live long enough
18 |     sort_by_key(&mut v, Country);
   |     ---------------------------- argument requires that `s1` is borrowed for `'static`
note: due to a current limitation of the type system, this implies a `'static` lifetime
 8 | fn sort_by_key<T, K>(arr: &mut [T], mut key: K) where for<'a> K: KeyProjection<'a, T> {
   |                                                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

Put `In: 'a` on the trait instead, and the impl for `Order<'s>` fails at once with
`error[E0478]: lifetime bound not satisfied`. The GAT states its outlives bounds per use
(`where In: 'a`), so it has neither trap. Use the GAT.

## Repeat the opaque return type in an RPITIT impl

Writing the concrete type in the impl is a public refinement of the trait's contract, and the
on-by-default `refining_impl_trait` lint says so:

```text
warning: impl trait in impl method signature does not match trait method signature
 5 |     fn project<'a>(&mut self, i: &'a In) -> impl Ord + use<'a, Self, In> where Self: 'a;
   |                                             ---------------------------- return type from trait method defined here
10 |     fn project<'a>(&mut self, i: &'a Order) -> Reverse<&'a String> where Self: 'a {
   |                                                ^^^^^^^^^^^^^^^^^^^
   = note: add `#[allow(refining_impl_trait)]` if it is intended for this to be part of the public API of this crate
   = note: `#[warn(refining_impl_trait_internal)]` (part of `#[warn(refining_impl_trait)]`) on by default
```

The example is crate-private, so the lint reports `refining_impl_trait_internal`. A trait and
type reachable from outside the crate report `refining_impl_trait_reachable`; both belong to the
`refining_impl_trait` group. Repeat `impl Ord + use<'a>` in the impl and the warning disappears.
When the concrete type really is part of your published API, put
`#[expect(refining_impl_trait, reason = "...")]` on the impl method, not the `#[allow]` that the
note suggests. The `rust-lints` skill, when it is installed, owns the workspace lint table that sets
`refining_impl_trait`.

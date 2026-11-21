# Method resolution and trait coherence

Read this file when a diff adds a `Deref` impl, an extension trait, a `downcast_ref` chain, a
pointer-forwarding impl (`impl Trait for &mut T`, `Box<T>`, `Rc<T>`), a blanket impl, or a second
conversion impl between the same two types. Every trap here is a resolution decision the compiler
makes for you. Three of them are silent: the code compiles and calls the wrong method, or calls
itself. The rest are hard build breaks that arrive in downstream code.

Each item states a severity, the rule, the failing shape with its exact error, and the working
shape. Measured on rustc 1.97.0, edition 2024, unless a section names rustc 1.98.1.

Contents:

- Method lookup walks the deref chain; trait solving does not
- `Deref` gives method reuse, never substitutability
- `DerefMut` turns a field borrow into a whole-`self` borrow
- A pointer-forwarding impl needs `?Sized` and a body that names the inner impl
- The method receiver decides which pointers can forward
- An extension trait shadows silently in both directions
- Two traits with one method name kill the dot call
- A downcast chain is a match with the exhaustiveness check removed
- `impl From<X> for Y` forecloses `impl TryFrom<X> for Y` for ever
- A blanket impl forecloses every other impl on the same `Self`
- `#[fundamental]` decides which wrappers can carry a foreign trait

---

## Method lookup walks the deref chain; trait solving does not

**Severity: WARNING**

A dot call and a trait bound use two different algorithms. Do not reason about one from the
other.

`receiver.method()` builds the autoderef chain: the receiver type, then each `Deref::Target`
after it. At every step the compiler tries three receiver shapes in order: the type by value,
then `&T`, then `&mut T`. For each shape it tries inherent methods first, then the methods of
traits in scope. The first shape that supplies one candidate wins. A trait method at step 0
therefore beats an inherent method at step 1. Inside one step, the receiver shape outranks
"inherent first": a trait method that takes `self` beats an inherent method that takes `&self`,
and a trait `&self` method beats an inherent `&mut self` method (Rust 1.99.1):

```rust,run
struct S;
impl S {
    fn who(&self) -> &'static str { "inherent &self" }
}
trait Who { fn who(self) -> &'static str; }
impl Who for S {
    fn who(self) -> &'static str { "trait self" }
}

struct M;
impl M {
    fn go(&mut self) -> &'static str { "inherent &mut self" }
}
trait Go { fn go(&self) -> &'static str; }
impl Go for M {
    fn go(&self) -> &'static str { "trait &self" }
}

fn main() {
    assert_eq!(S.who(), "trait self");
    assert_eq!(M.go(), "trait &self");
    assert_eq!(S::who(&S), "inherent &self"); // path syntax picks the inherent method
    let mut m = M;
    assert_eq!(M::go(&mut m), "inherent &mut self");
}
```

```rust
use std::ops::Deref;

struct Inner;
impl Inner {
    fn who(&self) -> &'static str { "Inner inherent" }
}

struct Wrapper(Inner);
impl Deref for Wrapper {
    type Target = Inner;
    fn deref(&self) -> &Inner { &self.0 }
}

trait Who { fn who(&self) -> &'static str; }
impl Who for Wrapper {
    fn who(&self) -> &'static str { "Wrapper trait" }
}

fn main() {
    // Step 0 is `Wrapper`. It has no inherent `who`, but `Who` applies at step 0,
    // so the trait method wins over the inherent method at step 1.
    assert_eq!(Wrapper(Inner).who(), "Wrapper trait");
    // Force step 1 with an explicit deref.
    assert_eq!((*Wrapper(Inner)).who(), "Inner inherent");
}
```

Trait-bound solving does not walk that chain. `Wrapper: Speak` holds for the exact type or it
does not hold. Unsizing to `dyn Speak` does not walk it either.

```rust
use std::ops::Deref;

struct Inner;
trait Speak { fn speak(&self) -> &'static str; }
impl Speak for Inner { fn speak(&self) -> &'static str { "inner" } }

struct Wrapper(Inner);
impl Deref for Wrapper {
    type Target = Inner;
    fn deref(&self) -> &Inner { &self.0 }
}

fn takes_inner(i: &Inner) -> &'static str { i.speak() }

fn main() {
    let w = Wrapper(Inner);
    assert_eq!(w.speak(), "inner");        // method lookup walks the chain
    assert_eq!(takes_inner(&w), "inner");  // deref coercion to a named type
}
```

Both lines below fail on the same types, with
`error[E0277]: the trait bound 'Wrapper: Speak' is not satisfied`:

```rust,compile_fail,E0277
use std::ops::Deref;
struct Inner;
trait Speak { fn speak(&self) -> &'static str; }
impl Speak for Inner { fn speak(&self) -> &'static str { "inner" } }
struct Wrapper(Inner);
impl Deref for Wrapper {
    type Target = Inner;
    fn deref(&self) -> &Inner { &self.0 }
}

fn generic<T: Speak>(t: &T) -> &'static str { t.speak() }

fn main() {
    let _ = generic(&Wrapper(Inner));      // E0277: `Wrapper: Speak` not satisfied
    let _d: &dyn Speak = &Wrapper(Inner);  // E0277: `Wrapper: Speak` not satisfied
}
```

Rule: when a wrapper must satisfy a bound, write the impl on the wrapper and delegate in the
body. Never expect `Deref` to supply it.

---

## `Deref` gives method reuse, never substitutability

**Severity: WARNING**

`Deref` makes a wrapper feel like its target at exactly two sites: a dot call, and a coercion
that names the target type. It never makes the wrapper a subtype. Rust has no subtyping between
data types.

These three sites keep failing after you add `Deref`:

| Site | Result |
|------|--------|
| `fn f<T: Trait>(t: &T)` with `impl Trait for Target` | E0277, the bound names the wrapper |
| `let d: &dyn Trait = &wrapper;` | E0277, unsizing needs a real impl |
| `Vec<Wrapper>` passed where `Vec<Target>` is expected | E0308, no coercion inside a generic |

Rule: implement `Deref` only on a smart pointer, or as the read-only `[T]` or `str` view of a
collection newtype that [`type-and-trait-traps.md`](type-and-trait-traps.md) allows. When you
want a wrapper to carry the target's trait impls, write delegating impls, or generate them with
a macro. Count the delegating impls before you commit: if the target has 30 trait impls, `Deref`
looks cheap and still gives you none of them at a bound.

---

## `DerefMut` turns a field borrow into a whole-`self` borrow

**Severity: WARNING**

The borrow checker splits a struct by field: `&mut ctx.a` and `&mut ctx.b` coexist. A `DerefMut`
impl removes that split for every field it reaches. `&mut ctx.frame` compiles to
`DerefMut::deref_mut(&mut ctx).frame`, which borrows all of `ctx`, so a second accessor on a
disjoint field is rejected:

```rust,compile_fail,E0499
use std::ops::{Deref, DerefMut};
struct Static { frame: u64 }
struct World { entities: Vec<u32> }
struct Ctx<S> { statics: S, world: World }
impl<S> Deref for Ctx<S> { type Target = S; fn deref(&self) -> &S { &self.statics } }
impl<S> DerefMut for Ctx<S> { fn deref_mut(&mut self) -> &mut S { &mut self.statics } }
impl<S> Ctx<S> { fn world_mut(&mut self) -> &mut World { &mut self.world } }

fn broken(ctx: &mut Ctx<Static>) {
    let f: &mut u64 = &mut ctx.frame;      // via `deref_mut`: borrows all of `ctx`
    let w: &mut World = ctx.world_mut();   // error[E0499]: cannot borrow `*ctx` as
    w.entities.push(*f as u32);            //   mutable more than once at a time
}
```

The same body written as `&mut ctx.statics.frame` plus `&mut ctx.world` compiles: those are
disjoint fields. No call order fixes the `DerefMut` version. Drop the impl, and expose both
halves through one splitting accessor that returns a tuple of `&mut`:

```rust
struct Static { frame: u64 }
struct World { entities: Vec<u32> }
struct Ctx<S> { statics: S, world: World }
impl<S> Ctx<S> {
    // One call, two disjoint `&mut`. The borrow checker splits the fields.
    fn split_mut(&mut self) -> (&mut S, &mut World) { (&mut self.statics, &mut self.world) }
}

fn ok(ctx: &mut Ctx<Static>) {
    let (s, w) = ctx.split_mut();
    w.entities.push(s.frame as u32);
}

fn main() {
    let mut ctx = Ctx { statics: Static { frame: 9 }, world: World { entities: vec![] } };
    ok(&mut ctx);
    assert_eq!(ctx.world.entities, [9]);
}
```

Rule: never implement `DerefMut` on a context or state wrapper that also hands out other fields
through inherent methods. The wrapper reads as convenient and costs the caller every disjoint
borrow.

---

## A pointer-forwarding impl needs `?Sized` and a body that names the inner impl

**Severity: CRITICAL**

`impl<H: Handler> Handler for &mut H` carries an implicit `H: Sized`. `dyn Handler` is unsized,
so `&mut dyn Handler: Handler` and `Box<dyn Handler>: Handler` never hold — and that is the only
case the forwarding pattern exists to serve. The impl itself compiles. The failure lands at a
distant call site. Measured on rustc 1.98.1:

```text
error[E0277]: the trait bound `&mut dyn Handler: Handler` is not satisfied
  |     process_request(h, Request);
  |     --------------- ^ the trait `Sized` is not implemented for `dyn Handler`
help: the trait `Handler` is conditionally implemented for `&mut H`
  | impl<H: Handler> Handler for &mut H {
  |      - unsatisfied requirement introduced here: `dyn Handler: Sized`
note: required for `&mut dyn Handler` to implement `Handler`
```

The fix is `+ ?Sized` on the impl, never a `Sized` bound on the caller.

The body is the second trap. Inside `impl<H: Handler + ?Sized> Handler for &mut H`, `self` has
type `&mut &mut H`, which is exactly the self type of that impl. Method probing tries the
receiver by value at step 0, so `self.handle(r)` binds to the impl it sits in:

```rust
// WRONG: `self.handle(r)` re-enters this impl. rustc emits
// `warning: function cannot return without recursing`, and nothing else.
struct Request;
trait Handler { fn handle(&mut self, r: Request); }
impl<H: Handler + ?Sized> Handler for &mut H {
    fn handle(&mut self, r: Request) { self.handle(r) }
}
```

A direct `h.handle(r)` on a `&mut dyn Handler` still prints the right answer, because there step
0 matches `<dyn Handler as Handler>::handle`. The recursion detonates only when the impl is
reached through a generic bound: measured `fatal runtime error: stack overflow, aborting`, exit
code 134. Name the inner impl instead, with `H::method(self, ..)` or `(**self).method(..)`:

```rust
struct Request;
trait Handler { fn handle(&mut self, r: Request) -> u32; }
struct My(u32);
impl Handler for My { fn handle(&mut self, _r: Request) -> u32 { self.0 } }

impl<H: Handler + ?Sized> Handler for &mut H {
    fn handle(&mut self, r: Request) -> u32 { H::handle(self, r) }
}
impl<H: Handler + ?Sized> Handler for Box<H> {
    fn handle(&mut self, r: Request) -> u32 { (**self).handle(r) }
}

fn run<H: Handler>(mut h: H) -> u32 { h.handle(Request) }

fn main() {
    let mut m = My(42);
    assert_eq!(run(&mut m as &mut dyn Handler), 42);
    let b: Box<dyn Handler> = Box::new(My(7));
    assert_eq!(run(b), 7);
}
```

Generate the set with a macro so no impl in it drifts:

```rust
macro_rules! impl_handler_for_refs {
    ($T:ident) => {
        impl<H: $T + ?Sized> $T for &mut H { /* one $T::method(self, ..) per method */ }
        impl<H: $T + ?Sized> $T for Box<H> { /* one $T::method(self, ..) per method */ }
    };
}
```

Rule: set `unconditional_recursion` to `deny` at the crate root as soon as a forwarding impl
lands. It is warn-by-default, and this is the bug it exists for.

With a forwarding impl in scope, do not take the same trait by value (`impl Handler`) in a
recursive method: each level adds one `&mut`, and `cargo build` fails with `reached the recursion
limit while instantiating` after `cargo check` and `cargo clippy` pass. Take `&mut W` with
`W: Handler + ?Sized`; the `rust-callback-bounds` skill, when it is installed, has the failing
example.

---

## The method receiver decides which pointers can forward

**Severity: WARNING**

Forwarding needs `Deref`, `DerefMut`, or a move out of the deref. Each receiver shape therefore
admits a different set of pointers, and the set is fixed the moment the trait ships.

| Receiver | Forwards to | `?Sized` |
|----------|-------------|----------|
| `&self` | `&T`, `&mut T`, `Box<T>`, `Rc<T>`, `Arc<T>` | on all five |
| `&mut self` | `&mut T`, `Box<T>` | on both |
| `self` | `Box<T>` only | impossible |

The rejections, measured on rustc 1.97.0:

| Impl you cannot write | Error |
|-----------------------|-------|
| `&mut self` for `Rc<T>` or `Arc<T>` | `error[E0596]: cannot borrow data in an 'Rc' as mutable`, with `help: trait 'DerefMut' is required to modify through a dereference` |
| `&mut self` for `&T` | `error[E0596]: cannot borrow '**self' as mutable, as it is behind a '&' reference` |
| `self` for `Box<T>` with `+ ?Sized` | `error[E0277]: the size for values of type 'T' cannot be known at compilation time`, with `note: all function arguments must have a statically known size` |
| `self` for `&mut T`, at any sizedness | `error[E0507]: cannot move out of '*self' which is behind a mutable reference` |

```rust
use std::rc::Rc;
use std::sync::Arc;

// `&self`: forwards to every pointer, `?Sized` throughout.
trait Shared { fn go(&self); }
impl<T: Shared + ?Sized> Shared for &T     { fn go(&self) { T::go(self) } }
impl<T: Shared + ?Sized> Shared for &mut T { fn go(&self) { T::go(self) } }
impl<T: Shared + ?Sized> Shared for Box<T> { fn go(&self) { T::go(self) } }
impl<T: Shared + ?Sized> Shared for Rc<T>  { fn go(&self) { T::go(self) } }
impl<T: Shared + ?Sized> Shared for Arc<T> { fn go(&self) { T::go(self) } }

// `&mut self`: only these two exist.
trait Uniq { fn go(&mut self); }
impl<T: Uniq + ?Sized> Uniq for &mut T { fn go(&mut self) { T::go(self) } }
impl<T: Uniq + ?Sized> Uniq for Box<T> { fn go(&mut self) { T::go(self) } }

// `self` by value: only `Box`, and the `?Sized` is gone with it.
trait Consume { fn go(self); }
impl<T: Consume> Consume for Box<T> { fn go(self) { T::go(*self) } }

fn main() {}
```

Rule: pick the receiver for the pointer set the callers need, before the trait ships. A
`self`-by-value method forces `Sized` on its one forwarding impl, so `Box<dyn Trait>` can never
satisfy the trait. Widening `&mut self` to `&self` later changes every implementor.

---

## An extension trait shadows silently in both directions

**Severity: CRITICAL**

An extension trait adds a method to a type you do not own. Method lookup then gives that method
a fixed priority against every other candidate, and the priority can change under you with no
warning and no error.

Direction 1: your extension method loses to a new inherent method whose receiver shape comes
at the same point or earlier in the order above. This block resolves `b.len()` to
`Measure::len`:

```rust
// The extension trait supplies `len`. Every call site resolves to it.
struct Buf(Vec<u8>);
trait Measure { fn len(&self) -> usize; }
impl Measure for Buf {
    fn len(&self) -> usize { self.0.len() }
}

fn main() {
    assert_eq!(Buf(vec![1, 2, 3]).len(), 3);
}
```

The owner of `Buf` then adds an inherent `len`. The build stays clean, no lint fires, and every
`b.len()` in your crate changes meaning:

```rust
struct Buf(Vec<u8>);
impl Buf {
    fn len(&self) -> usize { 999 }
}
trait Measure { fn len(&self) -> usize; }
impl Measure for Buf {
    fn len(&self) -> usize { self.0.len() }
}

fn main() {
    assert_eq!(Buf(vec![1, 2, 3]).len(), 999);       // the inherent method now
    assert_eq!(Measure::len(&Buf(vec![1, 2, 3])), 3); // UFCS reaches the trait
}
```

Direction 2: your extension method wins over an inherent method that the caller expects. An
extension trait implemented on a wrapper takes step 0 of the deref chain, so it beats the
target's inherent method of the same name. That is the first section of this file.

The receiver shape also cuts the other way. An extension method that takes `self` keeps
winning over a new inherent `&self` method of the same name, so the owner's addition changes
nothing at your call sites but `(&b).len()` and `Buf::len(&b)`.

Rule: give an extension-trait method a name that no inherent method on the type, and no
inherent method on any type in its deref chain, already uses. When the name must collide, call
it through UFCS (`Measure::len(&b)`) at every site, and add a test that asserts the value the
extension returns.

---

## Two traits with one method name kill the dot call

**Severity: CRITICAL**

Rust has no method overloading. Two traits in scope that both define `render` for the same type
make `id.render()` ambiguous. The call fails with `error[E0034]: multiple applicable items in
scope`, and it names every candidate:

```rust,compile_fail,E0034
struct Id(u64);

trait Pretty { fn render(&self) -> String; }
impl Pretty for Id { fn render(&self) -> String { format!("#{}", self.0) } }

trait Debugish { fn render(&self) -> String; }
impl Debugish for Id { fn render(&self) -> String { format!("Id({})", self.0) } }

fn main() {
    let id = Id(7);
    println!("{}", id.render()); // error[E0034]: multiple `render` found
}
```

rustc suggests `Debugish::render(&id)` and `Pretty::render(&id)`. Both compile. Neither is a
fix you can apply to code you do not own.

This makes a new method on a published extension trait a downstream build break. The consumer
does not have to call the new method. It only has to have another trait of the same method name
in scope on the same type.

Rule: treat a method name on a published trait as part of the public API. Do not add a method
with a common name — `len`, `id`, `name`, `render`, `parse`, `into_parts` — to a trait that
downstream crates implement or import. Where an ambiguity already exists, remove one of the two
methods; do not push UFCS onto every caller.

---

## A downcast chain is a match with the exhaustiveness check removed

**Severity: CRITICAL**

A chain of `if let Some(x) = obj.downcast_ref::<Concrete>()` arms is a hand-rolled match. The
compiler checks nothing about it. Add a fourth implementor and the chain still compiles, still
runs, and silently skips the new type. Measured with three implementors and two downcast arms:
the build emits no warning about the chain, and the program handles 2 of 3 values.

When code downcasts a trait object, two facts hold at once: the set of types is closed, and the
code needs the concrete data back. That is an enum.

```rust
// The set is closed and the code needs the concrete data. An enum gets an
// exhaustiveness check; a downcast chain does not.
enum Shape {
    Sq(f64),
    Circle(f64),
    Tri(f64, f64),
}

fn area(s: &Shape) -> f64 {
    match s {
        Shape::Sq(a) => a * a,
        Shape::Circle(r) => std::f64::consts::PI * r * r,
        Shape::Tri(b, h) => 0.5 * b * h,
    }
}

fn main() {
    let v = [Shape::Sq(2.0), Shape::Circle(1.0), Shape::Tri(3.0, 4.0)];
    let total: f64 = v.iter().map(area).sum();
    assert!(total > 13.0);
}
```

Delete one arm from that `match` and the build fails with
`error[E0004]: non-exhaustive patterns: '&Shape::Tri(_, _)' not covered`.

The swap is not free. `Box<dyn Shape>` is 16 bytes plus one heap allocation per element. The
three-variant enum above is 24 bytes inline, sized by its widest variant, so one large variant
grows every element. Keep `dyn Trait` when a downstream crate supplies implementors, because no
downstream crate can add a variant to your enum.

When `dyn Trait` must stay and the concrete type must come back, make `Any` a supertrait and
upcast `&dyn Trait` to `&dyn Any` (Rust 1.86 or later). The `rust-type-erasure` skill, when it is
installed, has the recipe and the `.type_id()`-on-a-smart-pointer pitfall.

When the value set is genuinely open, or the values borrow and `Any` therefore cannot key them
at all, the store is a design problem and not a taste problem. See `rust-type-erasure` for the
three-rung ladder and for the `'static` bound that `Any` puts on your caller.

---

## `impl From<X> for Y` forecloses `impl TryFrom<X> for Y` for ever

**Severity: CRITICAL**

`core` ships `impl<T, U: Into<T>> TryFrom<U> for T`. Your `From` impl therefore already
generates a `TryFrom` impl with `Error = Infallible`. A hand-written `TryFrom` for the same
pair collides with it:

```rust,compile_fail,E0119
struct Celsius(f64);
struct Kelvin(f64);

impl From<Celsius> for Kelvin {
    fn from(c: Celsius) -> Kelvin { Kelvin(c.0 + 273.15) }
}

impl TryFrom<Celsius> for Kelvin {
    type Error = &'static str;
    fn try_from(c: Celsius) -> Result<Kelvin, Self::Error> { todo!() }
}

fn main() {}
// error[E0119]: conflicting implementations of trait `TryFrom<Celsius>` for type `Kelvin`
//   = note: conflicting implementation in crate `core`:
//           - impl<T, U> TryFrom<U> for T where U: Into<T>;
```

There is no escape. The blanket impl lives in `core`, so you cannot remove it, and you cannot
add a negative bound. Removing your own `From` impl is a breaking change for every caller that
writes `.into()`.

Rule: decide once, before the impl lands, whether the conversion can fail. Write `From` only
when it is total for every value of `X`. Write `TryFrom` when any input is rejected. When a
mostly-total conversion needs one guarded variant, keep `From` and add a named constructor:

```rust
struct Celsius(f64);
struct Kelvin(f64);

// One conversion, one direction, one trait. `From` gives `Into` and a
// `TryFrom` with `Error = Infallible` for free.
impl From<Celsius> for Kelvin {
    fn from(c: Celsius) -> Kelvin { Kelvin(c.0 + 273.15) }
}

// The fallible constructor is a named method, not a second trait impl.
impl Kelvin {
    fn from_celsius_checked(c: Celsius) -> Result<Kelvin, &'static str> {
        if c.0 < -273.15 { Err("below absolute zero") } else { Ok(Kelvin::from(c)) }
    }
}

fn main() {
    let k: Kelvin = Celsius(0.0).into();
    assert!((k.0 - 273.15).abs() < 1e-9);
    let k2 = Kelvin::try_from(Celsius(0.0)).unwrap(); // Error = Infallible
    assert!((k2.0 - 273.15).abs() < 1e-9);
    assert!(Kelvin::from_celsius_checked(Celsius(-300.0)).is_err());
}
```

Find both impls on one pair:

```bash
rg "impl (Try)?From<" --type rust -n | sort -k2
```

---

## A blanket impl forecloses every other impl on the same `Self`

**Severity: CRITICAL**

Stable Rust has no specialisation. A blanket impl overlaps every instantiation it covers, so
every impl you might want later on the same `Self` type is `E0119`. Two shapes hit this.

**A bridge blanket impl excludes pointer forwarding.** `impl<T: Sink> Handler for T` and
`impl<H: Handler + ?Sized> Handler for &mut H` cannot coexist:

```rust,compile_fail,E0119
struct Request;
trait Handler { fn handle(&mut self, r: Request); }
trait Sink { fn send(&mut self, r: Request); }

// Pick ONE of these two impls. Together they are E0119.
impl<T: Sink> Handler for T { fn handle(&mut self, r: Request) { self.send(r) } }

impl<H: Handler + ?Sized> Handler for &mut H {
    fn handle(&mut self, r: Request) { H::handle(self, r) }
}
```

The note names the reason: `downstream crates may implement trait 'Sink' for type '&mut _'`.
Coherence reasons about what a downstream crate may do, not about what your crate did. So you
choose once: either your trait derives itself from another trait, or callers may pass `&mut h`
and `Box<h>`. An `Arc<h>` cannot forward an `&mut self` receiver; use `Arc` only with an
`&self` trait plus interior synchronization. You cannot have both blanket implementations.

**A blanket impl over a parameter blocks every later concrete impl.** `impl<S> Handler<S> for X`
is a one-way door for `X`:

```rust,compile_fail,E0119
struct Mouse;
struct ConcreteState;
struct Standalone;
trait InputHandler<S> { fn handle_mouse(&mut self, s: &mut S, m: &Mouse); }

impl<S> InputHandler<S> for Standalone { fn handle_mouse(&mut self, _: &mut S, _: &Mouse) {} }
// error[E0119]: conflicting implementations of trait `InputHandler<ConcreteState>`
//               for type `Standalone`
impl InputHandler<ConcreteState> for Standalone {
    fn handle_mouse(&mut self, _: &mut ConcreteState, _: &Mouse) {}
}
```

The conflict is per `Self` type. `impl<S> InputHandler<S> for Standalone` and
`impl<S: TimeState> InputHandler<S> for Timed` coexist without a complaint.

Rule: write `impl<S> Trait<S> for X` only for an `X` that ignores `S` for ever.
A positive bound such as `S: SomeCapability` narrows the blanket impl but does
not reserve a disjoint concrete case: stable Rust has no negative bound that
says the concrete type can never implement that capability. Use a distinct
`Self` type or newtype when a later concrete implementation must coexist.

---

## `#[fundamental]` decides which wrappers can carry a foreign trait

**Severity: WARNING**

The orphan rule treats `&T`, `&mut T`, `Box<T>`, and `Pin<P>` as transparent, because they are
`#[fundamental]` (Rust Reference, glossary). `&Local` therefore counts as a local type and the impl
is legal. `Rc`, `Arc`, `Vec`, and every other container are ordinary foreign types, so `Rc<Local>`
is a foreign type and the impl is an orphan. Measured on rustc 1.98.1:

```rust,run
use std::fmt;
use std::pin::Pin;
use std::rc::Rc;
pub struct Local(i32);

// ALLOWED: `&T`, `&mut T`, `Box<T>`, and `Pin<P>` are #[fundamental], so they count as local.
impl fmt::Display for &Local {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}
impl fmt::Display for Box<Local> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}
impl fmt::Display for Pin<&mut Local> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

// `impl fmt::Display for Rc<Local>` is E0117. Newtype the wrapper instead.
pub struct SharedLocal(Rc<Local>);
impl fmt::Display for SharedLocal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0.0) }
}

fn main() {
    assert_eq!(format!("{}", &Local(1)), "1");
    assert_eq!(format!("{}", Box::new(Local(3))), "3");
    let mut pinned = Local(4);
    assert_eq!(format!("{}", Pin::new(&mut pinned)), "4");
    assert_eq!(format!("{}", SharedLocal(Rc::new(Local(5)))), "5");
}
```

`Pin` has one extra trap. `std` already forwards `Display`, `Debug`, and `fmt::Pointer` from the
pointer inside it (`impl<P: Display> Display for Pin<P>`). Once `Box<Local>: Display` exists,
`impl Display for Pin<Box<Local>>` is E0119, not E0117.

`Rc<Local>`, `Arc<Local>`, and `Vec<Local>` each give the same rejection:

```text
error[E0117]: only traits defined in the current crate can be implemented for types defined outside of the crate
  |
  | impl fmt::Display for Arc<Local> {
  | ^^^^^^^^^^^^^^^^^^^^^^----------
  |                       |
  |                       `Arc` is not defined in the current crate
  |
  = note: impl doesn't have any local type before any uncovered type parameters
  = note: for more information see https://doc.rust-lang.org/reference/items/implementations.html#orphan-rules
  = note: define and implement a trait or new type instead
```

Rule: when a foreign trait must reach a refcounted local type, newtype the wrapper
(`struct SharedLocal(Rc<Local>)`), not the payload. Wrapping `Local` itself changes nothing,
because the impl still names `Rc` on the outside.

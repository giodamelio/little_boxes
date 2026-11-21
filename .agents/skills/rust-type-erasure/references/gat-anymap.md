# The GAT owner/element map in full

Deep material for rung three of the ladder in `../SKILL.md`. Read that first. Rung three is
exotic; take it only when the value set is open **and** the values borrow.

Contents:

- [The complete store](#the-complete-store): `get_mut`, and the parts that look optional
- [The Miri verdict](#the-miri-verdict): the commands, and what a clean run misses
- [Drop runs through an empty marker trait](#drop-runs-through-an-empty-marker-trait)
- [Attacks that the trait pair defeats](#attacks-that-the-trait-pair-defeats), and two it does not
- [The extractor exploit, in full](#the-extractor-exploit-in-full): the layer not to build
- [The helper bound, with its diagnostic](#the-helper-bound-with-its-diagnostic): the rustc
  suggestion not to apply
- [Shipping it as a library](#shipping-it-as-a-library): E0117, the impls to ship, and the newtype
  workaround

Quoted rustc output comes from rustc 1.98.1, edition 2024, aarch64-apple-darwin. Quoted Miri
output comes from Miri 0.1.0 on nightly 2026-05-15.

## The complete store

`get_mut` is the same cast against a `&mut self` borrow. Both accessors elide the output
lifetime, so both borrows come from `self`.

```rust
use std::any::TypeId;
use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

trait AnyDrop {}
impl<T> AnyDrop for T {}

trait Owner: 'static { type Element<'a>: Element<'a, Owner = Self>; }
trait Element<'a>: 'a { type Owner: Owner<Element<'a> = Self>; }

#[derive(Default)]
struct AnyMap<'a> {
    invariant: PhantomData<UnsafeCell<&'a mut ()>>,
    table: HashMap<TypeId, Box<dyn AnyDrop + 'a>>,
}

impl<'a> AnyMap<'a> {
    fn put<E: Element<'a>>(&mut self, value: E) {
        self.table.insert(TypeId::of::<E::Owner>(), Box::new(value));
    }

    fn get<E: Element<'a>>(&self) -> Option<&E> {
        let boxed = self.table.get(&TypeId::of::<E::Owner>())?;
        let erased: &(dyn AnyDrop + 'a) = Box::deref(boxed);
        // SAFETY: `E: Element<'a>` pins `<E::Owner as Owner>::Element<'a> == E`,
        // so the key selects exactly this type. The output borrow is elided from
        // `&self`, so no lifetime is detached.
        unsafe {
            (erased as *const (dyn AnyDrop + 'a)
                    as *const <E::Owner as Owner>::Element<'a>).as_ref()
        }
    }

    fn get_mut<E: Element<'a>>(&mut self) -> Option<&mut E> {
        let boxed = self.table.get_mut(&TypeId::of::<E::Owner>())?;
        let erased: &mut (dyn AnyDrop + 'a) = Box::deref_mut(boxed);
        // SAFETY: as `get`, and the `&mut self` borrow makes the result unique.
        unsafe {
            (erased as *mut (dyn AnyDrop + 'a)
                    as *mut <E::Owner as Owner>::Element<'a>).as_mut()
        }
    }
}

// A type that is its own tag. Legal when the type is already `'static`.
impl Owner for String { type Element<'a> = Self; }
impl<'a> Element<'a> for String { type Owner = Self; }

// A borrowed type needs a separate `'static` tag.
struct CowStrTag;
impl Owner for CowStrTag { type Element<'a> = Cow<'a, str>; }
impl<'a> Element<'a> for Cow<'a, str> { type Owner = CowStrTag; }

fn main() {
    let s = String::from("hello world");
    let mut map = AnyMap::default();
    map.put(Cow::from(&s));
    map.put(String::from("owned"));
    assert_eq!(map.get::<Cow<str>>().map(|c| &**c), Some("hello world"));
    if let Some(owned) = map.get_mut::<String>() { owned.push('!'); }
    assert_eq!(map.get::<String>().map(String::as_str), Some("owned!"));
}
```

Notes on the parts that look optional and are not:

- `PhantomData<UnsafeCell<&'a mut ()>>` makes `AnyMap<'a>` invariant in `'a`. It is load-bearing,
  not decoration. Every other field is covariant, so without the marker `&AnyMap<'long>` coerces
  to `&AnyMap<'short>`. A user element type with interior mutability then breaks the store from
  safe code alone. Give `Cell<&'a u32>` a tag, hand `&AnyMap<'short>` to a function that calls
  `c.set(short)`, and the long slot holds a dead reference. Measured on the covariant store: the
  native run prints a garbage value after the referent dies, and both borrow models report
  `error: Undefined Behavior: constructing invalid value of type &u32: encountered a dangling
  reference (use-after-free)`. Put the marker back and rustc rejects the same program with
  ``error[E0597]: `short` does not live long enough``.
- `get` must keep its elided output lifetime. Nothing in the compiler enforces this. Write
  `fn get<E: Element<'a>>(&self) -> Option<&'a E>` and the store compiles with no diagnostic and
  no warning, and returns a reference that outlives the map. Measured: the native run reads the
  value after the map is dropped, and both borrow models report `error: Undefined Behavior:
  constructing invalid value of type &std::borrow::Cow<'_, str>: encountered a dangling reference
  (use-after-free)`.
- `Box<dyn AnyDrop + 'a>` supplies the `E: 'a` bound. Without the `+ 'a` the box would demand
  `'static` again and the whole design collapses.
- `TypeId::of::<E::Owner>()` is legal because `Owner: 'static`. `TypeId::of::<E>()` is not.

## The Miri verdict

Run the default Stacked Borrows model first, then Tree Borrows as additional evidence:

```bash
cargo +nightly miri run --locked
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri run --locked
```

Both run clean, with no diagnostics. That is a clean run on this program, not a proof for every
program. The `rust-sanitizers-miri` skill, when it is installed, says what a clean Miri run does
not cover: a stubbed or skipped path, and foreign code, which needs `-Zsanitizer=address`.

The property the run cannot show is the compile-time rejection of a borrow shorter than the map.
Put a `Cow::from(&s2)` where `s2` dies inside an inner block, and the store refuses it:

```text
error[E0597]: `s2` does not live long enough
   |         map.put(Cow::from(&s2));
   |                           ^^^ borrowed value does not live long enough
   |     }
   |     - `s2` dropped here while still borrowed
```

## Drop runs through an empty marker trait

`AnyDrop` has no method. A trait object's vtable carries a drop slot regardless, so the concrete
`Drop` still runs, including on the value a `HashMap::insert` displaces. The probe asserts the
order:

```rust,run
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

trait AnyDrop {}
impl<T> AnyDrop for T {}

thread_local! { static LOG: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }

struct Noisy(&'static str);
impl Drop for Noisy {
    fn drop(&mut self) { LOG.with(|log| log.borrow_mut().push(self.0)); }
}

fn main() {
    let mut table: HashMap<TypeId, Box<dyn AnyDrop>> = HashMap::new();
    let key = TypeId::of::<Noisy>();
    table.insert(key, Box::new(Noisy("first")));
    table.insert(key, Box::new(Noisy("second")));   // the displaced "first" drops here
    LOG.with(|log| assert_eq!(*log.borrow(), ["first"]));
    drop(table);
    LOG.with(|log| assert_eq!(*log.borrow(), ["first", "second"]));
}
```

## Attacks that the trait pair defeats

Two `TypeId` keys that collide would let `get` cast to the wrong type. The trait pair makes a
collision unconstructible, so the map needs no runtime type check. The compiler rejects each
miswiring at the impl that writes it, not at a use site. A second tag that claims an element type
that is already claimed:

```rust,compile_fail,E0271
trait Owner: 'static { type Element<'a>: Element<'a, Owner = Self>; }
trait Element<'a>: 'a { type Owner: Owner<Element<'a> = Self>; }

struct Payload<'a>(&'a str);
struct TagA;
struct TagB;
impl Owner for TagA { type Element<'a> = Payload<'a>; }
impl<'a> Element<'a> for Payload<'a> { type Owner = TagA; }

impl Owner for TagB { type Element<'a> = Payload<'a>; }
```

```text
error[E0271]: type mismatch resolving `<Payload<'a> as Element<'a>>::Owner == TagB`
10 | impl Owner for TagB { type Element<'a> = Payload<'a>; }
   |                                          ^^^^^^^^^^^
note: expected this to be `TagB`
 8 | impl<'a> Element<'a> for Payload<'a> { type Owner = TagA; }
note: required by a bound in `Owner::Element`
```

Every one of these was attempted against the store above and rejected at compile time.

| Attack | Diagnostic |
| --- | --- |
| Two owner tags for one element type | `error[E0271]: type mismatch resolving <Payload<'a> as Element<'a>>::Owner == TagB` |
| A tag and its element impl that disagree on `'a`, such as `type Element<'a> = Payload<'a>` beside `impl<'a> Element<'a> for Payload<'static>` | `error: incompatible lifetime on type`, plus `error[E0308]: mismatched types ... lifetime mismatch` |
| An `Element<'static>`-only impl, to smuggle a fixed lifetime in | `error: incompatible lifetime on type`, with `note: ... introduces a 'static lifetime requirement` |
| Shrinking the map by value to shorten `'a` | `error: lifetime may not live long enough`, with `note: the struct AnyMap<'a> is invariant over the parameter 'a` |

Two attacks the trait pair does **not** defeat. Both are author discipline, not a compiler guard:

- A tag whose `Element<'x>` ignores `'x` compiles. `impl Owner for String { type Element<'a> = Self; }`
  above is exactly that shape, and every `'static` element needs it. The bijection proves key
  uniqueness. It does not prove that the element uses the lifetime. The extractor exploit below
  is built on this hole.
- A `get` that names `'a` in its output compiles. See the note on elision above.

## The extractor exploit, in full

This is the layer to **not** build. It is reproduced here so a reviewer can recognize the shape.
The program is complete and compiles without error on rustc 1.98.1. Do not run it outside Miri.

```rust
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

trait AnyDrop {}
impl<T> AnyDrop for T {}

trait Owner: 'static { type Element<'a>: Element<'a, Owner = Self>; }
trait Element<'a>: 'a { type Owner: Owner<Element<'a> = Self>; }

// The sound store from the top of this file, without `get_mut`.
#[derive(Default)]
struct AnyMap<'a> {
    invariant: PhantomData<UnsafeCell<&'a mut ()>>,
    table: HashMap<TypeId, Box<dyn AnyDrop + 'a>>,
}

impl<'a> AnyMap<'a> {
    fn put<E: Element<'a>>(&mut self, value: E) {
        self.table.insert(TypeId::of::<E::Owner>(), Box::new(value));
    }
    fn get<E: Element<'a>>(&self) -> Option<&E> {
        let boxed = self.table.get(&TypeId::of::<E::Owner>())?;
        let erased: &(dyn AnyDrop + 'a) = Box::deref(boxed);
        // SAFETY: `E: Element<'a>` pins `<E::Owner as Owner>::Element<'a> == E`.
        unsafe {
            (erased as *const (dyn AnyDrop + 'a)
                    as *const <E::Owner as Owner>::Element<'a>).as_ref()
        }
    }
}

impl Owner for String { type Element<'a> = Self; }
impl<'a> Element<'a> for String { type Owner = Self; }
impl Owner for u32 { type Element<'a> = Self; }
impl<'a> Element<'a> for u32 { type Owner = Self; }

// The unsound layer: `from_world` returns `Self`, detached from `world`.
trait Extractor {
    type Extracted: ExtractedType;
    unsafe fn from_world(world: &AnyMap) -> Self;
}
trait ExtractedType: 'static {
    type Extractor<'a>: Extractor<Extracted = Self>;
}

trait System { fn run(&mut self, world: &AnyMap); }
struct FuncSystem<F, Args>(F, PhantomData<Args>);

impl<F, A1: ExtractedType, A2: ExtractedType> System for FuncSystem<F, (A1, A2)>
where F: for<'a> FnMut(A1::Extractor<'a>, A2::Extractor<'a>) {
    fn run(&mut self, world: &AnyMap) {
        // The `for<'a>` bound is meant to stop the closure keeping these.
        let a1 = unsafe { A1::Extractor::<'_>::from_world(world) };
        let a2 = unsafe { A2::Extractor::<'_>::from_world(world) };
        (self.0)(a1, a2);
    }
}

struct App<'w>(AnyMap<'w>);
impl App<'_> {
    fn add_system<A1: Extractor, A2: Extractor, F>(&self, f: F)
    where F: FnMut(A1, A2), FuncSystem<F, (A1::Extracted, A2::Extracted)>: System {
        FuncSystem(f, PhantomData::<(A1::Extracted, A2::Extracted)>).run(&self.0);
    }
}

// The extension point. One impl whose GAT is constant in 'a turns the guarantee off.
struct Evil<R>(R);
struct EvilExtracted<S>(PhantomData<S>);

impl<S: 'static + for<'x> Element<'x>> Extractor for Evil<&'static S> {
    type Extracted = EvilExtracted<S>;
    unsafe fn from_world(world: &AnyMap) -> Self {
        let s = world.get::<S>().expect("must be in world") as *const S;
        unsafe { Evil(&*s) }
    }
}
impl<S: 'static + for<'x> Element<'x>> ExtractedType for EvilExtracted<S> {
    type Extractor<'a> = Evil<&'static S>;
}

fn main() {
    let mut stash: Option<&'static String> = None;
    {
        let mut m = AnyMap::default();
        m.put("Hello world".to_string());
        m.put(42_u32);
        let app = App(m);
        app.add_system(|e: Evil<&'static String>, _i: Evil<&'static u32>| {
            stash = Some(e.0);          // safe code, keeps a &'static into the world
        });
    }                                    // world dropped, buffer freed
    println!("resurrected: {:?}", stash.unwrap());
}
```

Observed: the native run reads freed memory, so its result varies by build (a garbage string or
a segfault). Miri reports the same use-after-free under both borrow models:

```text
$ cargo +nightly miri run
error: Undefined Behavior: constructing invalid value of type
std::option::Option<&std::string::String>: at .<enum-variant(Some)>.0,
encountered a dangling reference (use-after-free)
    |
    |     println!("resurrected: {:?}", stash.unwrap());
    |                                   ^^^^^ Undefined Behavior occurred here

$ MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri run
error: Undefined Behavior: ... encountered a dangling reference (use-after-free)
```

Two facts a reviewer usually gets wrong about this exploit:

1. The malicious `from_world` body is identical to the reference implementation's own body. The
   attack is in the `ExtractedType` impl, where the GAT drops `'a`, not in the `unsafe` block.
2. Adding the "optional" reverse constraint, `trait Extractor<'a>: 'a` with
   `type Extracted: ExtractedType<Extractor<'a> = Self>`, changes nothing. The expected
   `E0207: the lifetime parameter is not constrained` does not fire. E0207 asks only whether a
   parameter appears in the impl's trait reference **or** self type, and
   `impl<'a, S> Extractor<'a> for Evil<&'static S>` names `'a` in the trait reference. The GAT is
   still a constant function of `'a`. This is the compiled proof:

```rust
use std::marker::PhantomData;

trait Extractor<'a>: 'a { type Extracted: ExtractedType<Extractor<'a> = Self>; }
trait ExtractedType: 'static { type Extractor<'a>: Extractor<'a, Extracted = Self>; }

struct Evil<R>(R);
struct EvilExtracted<S>(PhantomData<S>);

// `'a` appears in the trait reference, so E0207 never fires, and the GAT below
// is still a constant function of `'a`. rustc emits dead-code warnings only.
impl<'a, S: 'static> Extractor<'a> for Evil<&'static S> { type Extracted = EvilExtracted<S>; }
impl<S: 'static> ExtractedType for EvilExtracted<S> { type Extractor<'a> = Evil<&'static S>; }

fn main() {}
```

The repair is a signature change, not a bound: make extraction return a lifetime-carrying
associated type, `fn from_world<'w, 'm>(world: &'w AnyMap<'m>) -> Self::Out<'w>`, so the compiler
ties the result to the world without any `unsafe`. Keep the two lifetimes apart: `&'w AnyMap<'w>`
borrows the invariant map until its destructor runs, and every call site fails with `E0597`.

## The helper bound, with its diagnostic

A helper that reads the map but does not know the map's `'a` cannot write `T: 'static`. It must
demand the element trait at every lifetime. Cut both `Evil` impl bounds in the exploit above to
`S: 'static`, and rustc reports:

```text
error[E0277]: the trait bound `S: Element<'_>` is not satisfied
   |         let s = world.get::<S>().expect("must be in world") as *const S;
   |                       ---   ^ the trait `Element<'_>` is not implemented for `S`
note: required by a bound in `AnyMap::<'a>::get`
   |     fn get<E: Element<'a>>(&self) -> Option<&E> {
help: consider further restricting type parameter `S` with trait `Element`
   | impl<S: 'static + Element<'_>> Extractor for Evil<&'static S> {
```

Do not apply that `help:` line. `'_` is not allowed in an impl bound, and rustc then reports
``error[E0637]: `'_` cannot be used here``. The bound that compiles is
`S: 'static + for<'x> Element<'x>`, as the exploit uses. Adding `'static` alone does not help:
`get` is quantified over the map's `'a`, which the helper impl never names.

## Shipping it as a library

Two crates, `map_lib` (defines `Owner`, `Element`, `AnyMap`) and a user crate that depends on it.
The user crate cannot register a foreign type:

```rust,ignore
use std::borrow::Cow;
use map_lib::{AnyMap, Element, Owner};

impl<'a> Element<'a> for Cow<'a, str> { type Owner = MyCowTag; }
```

```text
error[E0117]: only traits defined in the current crate can be implemented for types defined outside of the crate
4 | impl<'a> Element<'a> for Cow<'a, str> { type Owner = MyCowTag; }
  | ^^^^^^^^^^^^^^^^^^^^^^^^^------------
  |                          `Cow` is not defined in the current crate
```

The newtype workaround compiles in the user crate, and the assertion holds. The block prints
nothing:

```rust,ignore
// user crate
pub struct MyCow<'a>(pub Cow<'a, str>);
pub struct MyCowTag;

impl Owner for MyCowTag { type Element<'a> = MyCow<'a>; }
impl<'a> Element<'a> for MyCow<'a> { type Owner = MyCowTag; }

fn main() {
    let s = String::from("downstream");
    let mut m = AnyMap::default();
    m.put(MyCow(Cow::from(&s)));
    assert_eq!(m.get::<MyCow>().map(|c| &*c.0), Some("downstream"));
}
```

Consequences for a published map crate. Budget for them before you publish:

- Ship `Owner` and `Element` impls for every std type users will store: `Cow<'a, str>`,
  `Cow<'a, [u8]>`, `&'a str`, `&'a [u8]`, `String`, and the integers.
- Document the newtype workaround above for everything else.
- Do not offer a `register::<T>()` macro to hide the error. It does not remove it.

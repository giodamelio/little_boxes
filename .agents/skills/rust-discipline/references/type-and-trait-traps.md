# Type-system and trait traps

Read this file when a diff adds a trait impl, a newtype, a lifetime parameter, or a blanket impl
on a public trait.
Each trap is silent: the code compiles, and the defect appears later at run time, at a downstream
consumer, or on a different target.

Each item states a severity, a wrong example, a correct example, and a rule. Traps that come from
the shape of the data — hash keys, text reversal, and large arrays — live in
[data-shape-traps.md](data-shape-traps.md). What each parameter bound accepts and rejects lives in
[argument-shapes.md](argument-shapes.md). `impl Drop` makes every move of a non-`Copy` field
E0509; [drop-and-raii.md](drop-and-raii.md) has the full cost and the guard pattern. Builder
receivers, `Clone` on handles, mutexes, `Arc` and `Weak`, and integer overflow live in
[ownership-and-concurrency-traps.md](ownership-and-concurrency-traps.md).

Contents:

- `Deref` on a non-pointer type causes method collision
- Borrowed cache values must outlive their use
- A `Cow` field infects the struct with a lifetime
- A blanket impl in a public API is a semver hazard
- A blanket impl on an empty trait is not a bound alias
- A `Clone` supertrait or a `-> Self` method destroys dyn compatibility
- A trait bound on a struct definition is viral and guarantees nothing

---

## `Deref` on a non-pointer type causes method collision

**Severity: WARNING**

`Deref<Target = T>` on a newtype that is not a smart pointer exposes every method of `T`
through auto-deref. This creates silent method shadowing, and it creates a semver break when
either type gains a method.

```rust
// BAD: Deref on a plain domain newtype
struct UserId(u64);
impl std::ops::Deref for UserId {
    type Target = u64;
    fn deref(&self) -> &u64 { &self.0 }
}
// UserId now exposes every u64 method through auto-deref, and leaks the representation.
```

`Deref` gives method reuse, never substitutability: method lookup and `&Target` coercion follow
the chain, trait-bound solving and unsizing to `dyn Trait` do not.

Rule: implement `Deref` only on a smart pointer. For a domain newtype, write explicit accessor
methods, or implement `AsRef` and `From`. Exception: a collection newtype may implement
`Deref<Target = [T]>` or `Deref<Target = str>` as a read-only view, as `Vec` and `String` do.
Never add `DerefMut` to it, and never target the owning collection. The `rust-iterator-impl`
skill uses this shape. For polymorphism, declare a trait and implement it on
each type: `Vec<Box<dyn T>>`, a `&dyn T` parameter, and a generic bound each need the impl, and
none of them accepts a `Deref` in its place. The compiler reports nothing at the point the mistake
is made, only at the first polymorphic use site, by which time the rewrite is a full API change.
[trait-resolution.md](trait-resolution.md) holds the resolution order, the three sites that keep
failing after you add `Deref`, and the exact E0277 each one gives.

---

## Borrowed cache values must outlive their use

**Severity: WARNING**

A borrowed cache requires every stored reference to remain valid until its last use.
A shared lifetime on the input and cached values states that contract correctly. Different
input lifetimes can shorten to the common cache lifetime:

```rust,run
use std::collections::HashMap;

fn first_word<'a>(s: &'a str, cache: &mut HashMap<String, &'a str>) -> &'a str {
    if let Some(cached) = cache.get(s) { return cached; }
    let word = s.split_whitespace().next().unwrap_or("");
    cache.insert(s.to_owned(), word);
    word
}

fn main() {
    let first = String::from("first source");
    let second = String::from("second source");
    let mut cache = HashMap::new();
    assert_eq!(first_word(&first, &mut cache), "first");
    { assert_eq!(first_word(&second, &mut cache), "second"); }
    assert_eq!(cache.len(), 2);
}
```

Writing separate `'input: 'cache` parameters expresses the same outlives requirement. It
cannot make a short-lived source survive a later cache lookup:

```rust,compile_fail,E0597
use std::collections::HashMap;

fn remember<'cache, 'input: 'cache>(
    input: &'input str,
    cache: &mut HashMap<u8, &'cache str>,
) {
    cache.insert(0, input);
}

fn main() {
    let mut cache = HashMap::new();
    {
        let input = String::from("temporary");
        remember(&input, &mut cache);
    }
    assert_eq!(cache.get(&0), Some(&"temporary"));
}
```

Keep the source owners alive until the cache's last use, shorten the cache's scope, or store
owned values. An owned cache returns a reference tied to the cache borrow, not to the input:

```rust,run
use std::collections::HashMap;

fn first_word<'c>(s: &str, cache: &'c mut HashMap<String, String>) -> &'c str {
    cache.entry(s.to_owned())
        .or_insert_with(|| s.split_whitespace().next().unwrap_or("").to_owned())
        .as_str()
}

fn main() {
    let mut cache = HashMap::new();
    {
        let input = String::from("temporary source");
        assert_eq!(first_word(&input, &mut cache), "temporary");
    }
    assert_eq!(cache.get("temporary source").map(String::as_str), Some("temporary"));
}
```

Run both passing probes and the E0597 probe after changing a cache signature. They distinguish
an ownership requirement from an annotation change; neither annotation extends an owner's life.

---

## A `Cow` field infects the struct with a lifetime

**Severity: WARNING**

The same failure family as the section above, from one field. A `Cow<'a, str>` field puts `'a` on
the struct and on every signature that touches it, exactly like a `&'a mut T` field.

The `rust-copy-on-write` skill holds the whole decision: the E0515 and E0521 shapes, the
`into_static` exit, and the hit rate that decides whether the field is worth a lifetime at all.

---

## A blanket impl in a public API is a semver hazard

**Severity: CRITICAL**

`impl<T: Foo> Bar for T` on a `pub trait Bar` lets a downstream crate write
`impl Bar for MyType` only while `MyType: !Foo`. When a later minor release adds another
blanket impl, narrows a bound, or implements `Foo` for a type the downstream crate uses,
downstream compilation breaks. The error appears in the consumer's CI, months after you ship
the change.

```rust
use std::fmt::Display;

// HAZARD: a downstream `impl Bar for MyType where MyType: Display`
// can conflict with this on a future version bump.
pub trait Bar { fn bar(&self) -> String; }
impl<T: Display> Bar for T {
    fn bar(&self) -> String { format!("{}", self) }
}
```

```rust
use std::fmt::Display;

// SAFE: the trait is sealed, so no downstream impl can ever exist,
// and the blanket impl is therefore free of conflict risk.
mod private {
    pub trait Sealed {}
    // The sealed set. Without an impl here, `Bar` holds for no type,
    // and `1u8.bar()` fails with E0599.
    impl<T: std::fmt::Display> Sealed for T {}
}
pub trait Bar: private::Sealed { fn bar(&self) -> String; }
impl<T: Display> Bar for T {
    fn bar(&self) -> String { format!("{}", self) }
}

pub fn label() -> String { 1u8.bar() }
```

Rule: allow a blanket `impl<T: ...> PubTrait for T` only when `PubTrait` is sealed, that is,
it extends a private trait. Otherwise write explicit per-type impls. Going from sealed to
unsealed later is free. Going from blanket to explicit later is a downstream-breaking semver
bump.

Find candidates:

```bash
rg "^impl<T(:\s*\w[\w:+ ]*)?>\s+\w+\s+for\s+T\b" --type rust -n
```

---

## A blanket impl on an empty trait is not a bound alias

**Severity: WARNING**

`trait Featured {}` plus `impl<T: Clone + Debug> Featured for T {}` names a bound set and
propagates nothing. Generic code over `T: Featured` cannot call any method of `Clone` or `Debug`.
The first call fails with `error[E0599]: no method named 'clone' found for type parameter 'T' in
the current scope`, and the help suggests restricting the parameter again, which is the whole cost
the alias was meant to remove. A supertrait declaration propagates every bound to each use site,
and the same blanket impl still applies the trait to every qualifying type automatically.

```rust
use std::fmt::Debug;

// An empty trait plus a blanket impl names a bound set and propagates nothing.
// `T: Featured` alone cannot call `clone`.
trait Featured {}
impl<T: Clone + Debug> Featured for T {}

// A supertrait propagates every bound to each use site. The same blanket impl
// still applies the trait to every qualifying type.
trait Alias: Clone + Debug {}
impl<T: Clone + Debug> Alias for T {}

fn duplicate<T: Alias>(value: T) -> (T, T) {
    let copy = value.clone();
    (value, copy)
}
```

Rule: write the bound set as a supertrait list, never as an empty trait with a blanket impl. The
supertrait form is not free in the other direction: adding a supertrait to a published trait breaks
every existing impl, so declare the full list in the first release. The blanket impl itself still
carries the conflict hazard of the section above, so seal the trait.

---

## A `Clone` supertrait or a `-> Self` method destroys dyn compatibility

**Severity: WARNING**

Each half removes `dyn Trait` on its own, and each gives `error[E0038]`. `Clone` requires
`Self: Sized`, so no vtable can be built. A method that names `Self` in return position gets no
vtable slot. `Debug` alone stays dyn compatible.

The error lands at every `dyn Render` and `Box<dyn Render>` use site, never at the trait
definition, so a one-word edit to the supertrait list breaks code that never mentions `Clone`.

```rust
use std::fmt::Debug;

// `trait Render: Clone + Debug` has no `dyn Render`. `where Self: Sized` on the
// method that names `Self` keeps the rest of the trait usable behind `dyn`.
trait Render: Debug {
    fn draw(&self);

    fn duplicate(&self) -> Self
    where
        Self: Sized;
}

#[derive(Clone, Debug)]
struct Dot;

impl Render for Dot {
    fn draw(&self) {}

    fn duplicate(&self) -> Self {
        self.clone()
    }
}

fn draw_all(items: &[Box<dyn Render>]) {
    for item in items {
        item.draw();
    }
}
```

Rule: never put `Clone` in the supertrait list of a trait you intend to use behind `dyn`, and add
`where Self: Sized` to every method that names `Self` in return position. That clause has a cost:
the method cannot be called through `dyn Trait` at all. The `rust-compiler-errors` skill
holds the E0038 triage — a table of the shapes that remove the vtable, the `...because` note that
each one prints, and the `clone_box` replacement for a `Clone` supertrait.

---

## A trait bound on a struct definition is viral and guarantees nothing

**Severity: WARNING**

`struct Container<T: Featured>` does not reach the caller as a guarantee. It only forces every impl
block, every function signature, and every derive on that type to repeat the bound. An impl block
that omits it fails with `error[E0277]: the trait bound 'T: Featured' is not satisfied` pointing at
the impl header, with `note: required by a bound in 'Container'`. The reflexive fix, copying the
bound onto that impl too, spreads the constraint through the file until an unrelated caller cannot
satisfy it.

```rust
trait Featured: Clone {}

// No bound on the definition. Every impl block is then free to omit it.
struct Container<T> {
    item: T,
}

impl<T> Container<T> {
    fn get(&self) -> &T {
        &self.item
    }
}

// The bound sits on the one impl that needs it.
impl<T: Featured> Container<T> {
    fn duplicate(&self) -> T {
        self.item.clone()
    }
}
```

Rule: declare the type parameter bare, and put each bound on the impl block that needs it. Three
cases need the bound on the definition. First, a `where` clause that an associated type or a const
generic expression depends on. Second, a type whose `Drop` impl needs the bound: E0367 rejects a
`Drop` impl with bounds that the struct lacks. Third, a closure-typed field such as
`struct S<F: Fn(&str) -> usize> { f: F }`: the bound gives the closure its signature at the struct
literal, and without it `S { f: |s| s.len() }` is E0282. Do not remove such a bound blindly.
Removing a bound from a published struct is not a breaking change; adding one is.

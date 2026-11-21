---
description: Use when designing or reviewing type erasure or a type map keyed by TypeId — Box<dyn Any>, downcast_ref, an anymap, an extensions map, a resource registry, an ECS-style world — above all when the stored values borrow and are not 'static. Also for upcasting dyn Trait to dyn Any, E0597 blamed on the caller of an Any-bounded map, a type_name used as a key, the GAT bijection that keys borrowed values, an extractor or system param that detaches a lifetime, and E0117 when shipping the pattern as a library.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-type-erasure
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: b79be7bb8d33b1bc8ae56a85bc45e96acf1a2848
name: rust-type-erasure
---
# Rust type erasure

`Any` keys nothing that borrows. Its only impl is `impl<T: 'static + ?Sized> Any for T`, and the
error for that bound lands on the caller. Erase no more than [the ladder](#the-ladder) needs.

Quoted rustc output comes from rustc 1.98.1, edition 2024, aarch64-apple-darwin.

## Route the symptom to a section

| Symptom or task | Section |
| --- | --- |
| `error[E0597]: ... argument requires that s is borrowed for 'static`, or `'a must outlive 'static` | [`Any` is bound to `'static`](#any-is-bound-to-static) |
| `type_name` as a key because `TypeId::of` refused | [`type_name` is not a type key](#type_name-is-not-a-type-key) |
| Choosing an enum, a `dyn Any` map, or something exotic | [The ladder](#the-ladder) |
| `.type_id()` on a `Box`, `Arc`, or `Rc` never equals `TypeId::of::<T>()`, or a planned `fn as_any` | [Get the concrete type back](#get-the-concrete-type-back-from-dyn-trait) |
| You must store a borrow such as `Cow<'a, str>`, keyed by type | [Rung 3](#rung-3-an-open-set-of-borrowed-values) |
| `error[E0271]` on `<P<'a> as Element<'a>>::Owner == Tag`, or `incompatible lifetime on type` on an `Owner` or `Element` impl | [The bijection](#the-bijection-is-the-collision-proof) |
| `error[E0277]: the trait bound S: Element<'_> is not satisfied` in a helper | [The helper bound is `for<'x>`](#the-helper-bound-is-forx-not-static) |
| An extractor, a system param, or a `from_world` | [Where it breaks](#where-it-breaks-an-extractor-that-detaches-the-lifetime) |
| `error[E0117]: only traits defined in the current crate ...` in a user crate | [The orphan rule](#the-orphan-rule-caps-the-pattern-at-one-crate) |

## Verify the store

Rungs one and two need only ordinary tests. Run these checks when you add or change a
lifetime-carrying store (rung three), its accessors, its extraction layer, or its `unsafe` cast:

1. The `rust-sanitizers-miri` skill, when installed, owns the Miri policy. With a nightly miri
   component (`rustup +nightly component add miri`), run `cargo +nightly miri test --locked` on
   the store's tests: this Stacked Borrows run is the gate. Then run
   `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test --locked` as extra evidence, never as
   the only gate. Without Miri, report the cast as not checked by Miri.
2. Pin each rejection with a `compile_fail` doctest on public items: a borrow shorter than the
   map in `put`, a `get` result used after the map drops, a `Cell` element shortened through
   `&AnyMap<'short>`, a second tag for one element type, and a system closure that stores its
   argument outside (`E0521`). Run `cargo +nightly test --doc --locked` to enforce each named
   code, which stable ignores; without nightly, run `cargo check` on a probe crate per code.
3. Run `cargo clippy --locked --all-targets -- -D warnings`. `clippy::type_id_on_box` catches
   `.type_id()` on a `Box` only.
4. Review each extraction signature: no function returns `Self`, or any type that does not name
   the input borrow's lifetime, from a `&` argument. No test or Miri run catches that edit.

Miri checks only the paths the tests execute. A `get` that returns `&'a E`, or a map without the
invariance marker, passes every test and every Miri run of correct code; only the step 2
doctests catch that edit. Keep each doctest to the one failure it guards.

## `Any` is bound to `'static`

`TypeId::of::<T>` carries the same `T: 'static` bound as the `Any` impl. `TypeId::of::<&'a str>()`
inside `fn f<'a>()` gives `lifetime may not live long enough ... 'a must outlive 'static`.

A generic `put<S: Any>` compiles clean; region inference moves the requirement outward, so
`map.put(Cow::from(&s))` fails with E0597 on the local `s`. Read the trailing
`note: requirement that the value outlives 'static introduced here`, not the span: it names the
bound. A longer-lived `s` never fixes this shape. Change the store instead. Read
[references/any-probes.md](references/any-probes.md#the-static-bound-fails-at-the-caller) when you
need the full program and diagnostic.

## `type_name` is not a type key

`std::any::type_name::<T>()` has no `T: 'static` bound, so it compiles where `TypeId::of`
refused. The string is not a unique identifier, and rustc erases lifetimes before code
generation, so two distinct types share one string (as of Rust 1.99.1, `Wrapper<'_>` and `&str`;
probe in [references/any-probes.md](references/any-probes.md#two-types-share-one-type_name)).

Never key a map, a registry, or a cache on `type_name`, and never compare two `type_name` strings
to decide a cast, because a collision casts to the wrong type or extends a lifetime. Use it in a
panic message, a log line, or an error string.

## The ladder

Stop at the first rung that holds, usually rung one or two.

| Value set | Value ownership | Use |
| --- | --- | --- |
| Closed: you name every type | owned or borrowed | A lifetime-parameterized enum. No `unsafe`, exhaustive `match` |
| Open: other modules register types | owned, or shared behind `Arc` | `HashMap<TypeId, Box<dyn Any>>`. No `unsafe` |
| Open | borrows a buffer that outlives the map | The GAT owner/element bijection. One `unsafe` block |

Take rung three only when the value set is open **and** the values borrow.

### Rung 1: a closed value set

An enum parameterized by the lifetime holds borrowed data with no erasure; a new variant breaks
every `match` with `E0004`. [Example](references/ladder-examples.md#rung-1-a-lifetime-parameterized-enum).

### Rung 2: an open set of owned values

A request-extension map or a resource registry stores owned values or `Arc` clones, so the
`'static` bound costs nothing. Key on `TypeId::of::<T>()`, store `Box<dyn Any + Send + Sync>`, and
read with `downcast_ref`. [Example](references/ladder-examples.md#rung-2-an-extensions-map).

Before you climb to rung three, replace the borrow with `Arc<T>` or an owned copy at the map
boundary. The `rust-copy-on-write` skill, when installed, has the allocation counts; an
`Arc<str>` clone allocates nothing.

### Get the concrete type back from `dyn Trait`

Make `Any` a supertrait and let the compiler upcast `dyn Trait` to `dyn Any` (stable since 1.86).
Do not add a `fn as_any(&self) -> &dyn Any` method to every impl unless the MSRV is below 1.86.

Dereference the smart pointer before you ask for a `TypeId`. `boxed.type_id()` returns the
`TypeId` of the `Box`, which is itself `Any`, so a comparison or a hand-written downcast fails
silently. `clippy::type_id_on_box` flags only `Box`; for `Arc` and `Rc`, write
`(*ptr).type_id()` or upcast to `&dyn Any` first
([probe](references/any-probes.md#upcast-and-downcast-through-a-box)).

`trait Plugin: Any` makes every implementor `'static`; a plugin that borrows needs the ladder.

### Rung 3: an open set of borrowed values

Key on a `'static` **owner tag**, not on the element type, which carries the lifetime. Tie them
with two mutually constraining GATs:
`trait Owner: 'static { type Element<'a>: Element<'a, Owner = Self>; }` is the key, and
`trait Element<'a>: 'a { type Owner: Owner<Element<'a> = Self>; }` is the value. The map stores
`Box<dyn AnyDrop + 'a>` keyed by `TypeId::of::<E::Owner>()`.

Read [references/gat-anymap.md](references/gat-anymap.md) when you implement or review rung
three: it has the complete store, the Miri verdict, the drop probe, and every attack with its
diagnostic.

Four properties carry the proof; an edit that drops one breaks it:

- `get` returns `Option<&E>` with an elided output lifetime, so the borrow is the `&self` borrow.
  Never write `-> Option<&'a E>`: it compiles silently and returns a reference that outlives the
  map.
- `put` takes `E` by value at the map's `'a`, and `Box<dyn AnyDrop + 'a>` forces `E: 'a`.
- `AnyDrop` is an empty marker trait, not `Any`, which would re-impose `'static`. Every vtable
  carries a drop slot, so `Box<dyn AnyDrop + 'a>` still runs the concrete `Drop`.
- `PhantomData<UnsafeCell<&'a mut ()>>` makes the map invariant in `'a`. Without it, safe code
  writes a short borrow into a long slot through interior mutability.

The `SAFETY` comment on the cast in `get` must name the GAT equality
`<E::Owner as Owner>::Element<'a> == E`: it is the whole proof that the key selects the type.

## The bijection is the collision proof

The trait pair makes a key collision unconstructible, so `get` needs no runtime type check. rustc
rejects each miswiring at the impl: a second tag for a claimed element type gives `error[E0271]`, and a
tag and element impl that disagree on `'a`, or an `Element<'static>`-only impl, give
`error: incompatible lifetime on type`. Fix the impl, or give the second use its own newtype and
tag; do not weaken the trait pair. The
[attack table](references/gat-anymap.md#attacks-that-the-trait-pair-defeats) has each diagnostic.

The bijection does not prove that the element uses its lifetime: every `'static` element needs
`impl Owner for String { type Element<'a> = Self; }`, which ignores `'a`. The extractor below
exploits that hole.

## The helper bound is `for<'x>`, not `'static`

A helper that reads the map but does not know the map's `'a` must demand the element trait at
every lifetime. With `S: 'static` alone, `get` fails with
`error[E0277]: the trait bound S: Element<'_> is not satisfied`. Do not apply rustc's `help:`
bound `S: 'static + Element<'_>`: `'_` is not allowed there (`error[E0637]`). The bound that
compiles:

```rust
impl<S: 'static + for<'x> Element<'x>> Extractor for State<&S> { /* ... */ }
```

## Where it breaks: an extractor that detaches the lifetime

The map is sound; a layer that hands closures their arguments (an extractor, a system param, a
`from_world`) loses soundness. `F: for<'a> FnMut(A::Extractor<'a>)` is vacuous when
`Extractor<'a>` is constant in `'a`: one safe user impl hands the closure a `&'static` into the
map, a use-after-free. No bound forces a GAT to use its lifetime.

The rule: an extraction API returns a type that names the input borrow, for example
`fn from_world<'w, 'm>(world: &'w AnyMap<'m>) -> Self::Out<'w>`. Keep the two lifetimes apart:
`&'w AnyMap<'w>` borrows the invariant map until it drops (E0597 at every caller), and removing
the invariance marker to silence that makes the map unsound. An `unsafe fn` that returns `Self`
sets an obligation no caller can discharge. Read the
[exploit and the reverse-constraint proof](references/gat-anymap.md#the-extractor-exploit-in-full)
when you review an extraction layer.

## The orphan rule caps the pattern at one crate

Coherence blocks a downstream crate from registering a foreign type:
`impl<'a> Element<'a> for Cow<'a, str>` there is `error[E0117]`. A published map crate ships the
`Owner` and `Element` impls for the std types its users store, and documents a local newtype for
the rest. Read
[Shipping it as a library](references/gat-anymap.md#shipping-it-as-a-library) when you publish
the pattern.

## Related skills

When installed: `rust-discipline` decides whether to erase at all (a `downcast_ref` chain is a
`match` without the exhaustiveness check), `rust-unsafe` reviews the pointer cast and its
`// SAFETY:` comment, `rust-variance` explains the invariance marker, and `rust-compiler-errors`
covers E0597, E0277, and E0271 beyond these shapes.

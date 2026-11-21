# E0038: the trait is not dyn compatible

Read this when rustc reports E0038, or when a `where Self: Sized` item fails to compile through
`dyn Trait`. The triage table is in [SKILL.md](../SKILL.md).

`dyn Trait` needs a vtable. One item that cannot get a vtable slot removes the whole trait from
`dyn` use, so the error points at the `dyn Trait` type and not at the call that broke:

```text
error[E0038]: the trait `NoSelf` is not dyn compatible
note: for a trait to be dyn compatible it needs to allow building a vtable
   |     fn describe() -> String;
   |        ^^^^^^^^ ...because associated function `describe` has no `self` parameter
```

Read the `...because` note first. rustc prints one note per shape; these eleven are the common
ones.

| Shape in the trait | The `...because` note |
| --- | --- |
| `fn describe() -> String;` | ...because associated function `describe` has no `self` parameter |
| `fn go<T: Copy>(&self, t: T);` | ...because method `go` has generic type parameters |
| `fn ser(&self, out: impl Write);` | ...because method `ser` has generic type parameters |
| `fn dup(&self) -> Self;` | ...because method `dup` references the `Self` type in its return type |
| `fn eq_me(&self, other: &Self) -> bool;` | ...because method `eq_me` references the `Self` type in this parameter |
| `fn it(&self) -> impl Iterator<Item = u8>;` | ...because method `it` references an `impl Trait` type in its return type |
| `async fn m(&self) -> u32;` | ...because method `m` is `async` |
| `const N: usize;` | ...because it contains associated const `N` |
| `type Item<T>;` | ...because it contains generic associated type `Item` |
| `trait T: Clone` or `trait T: Sized` | ...because it requires `Self: Sized` |
| `trait T: PartialEq<Self>` | ...because it uses `Self` as a type parameter |

Row two and row three print the same note for signatures that look nothing alike. An
argument-position `impl Trait` is a hidden generic parameter: `fn ser(&self, out: impl Write)` is
`fn ser<W: Write>(&self, out: W)`. Take `&mut dyn Write` when a consumer may need `Box<dyn Trait>`.

The last four rows name no method, so they read like a different error. `Clone` has `Sized` as
a supertrait, so `: Clone` on the trait alone removes the vtable.

## Fix one item with `where Self: Sized`

The first seven rows and the generic associated type row sit on one item, and one clause on that
item is the whole fix:

```rust
pub trait Codec {
    fn decode(&self, src: &[u8]) -> Vec<u8>;   // keeps its vtable slot
    fn name() -> &'static str
    where
        Self: Sized;                           // leaves the vtable
    type Frame<T>
    where
        Self: Sized;                           // a GAT leaves the check the same way
}
```

`Vec<Box<dyn Codec>>` now compiles. Each implementor still calls `name()` through its concrete
type.

The clause on a GAT needs Rust 1.72 or later. On an older MSRV, a GAT makes the trait not dyn
compatible, even with the clause.

Know the cost before you type the clause: the item leaves the trait-object API. A later call
through `dyn` fails at the call site, not at the trait definition, and the message never mentions
dyn compatibility:

```text
error[E0277]: the size for values of type `dyn Codec` cannot be known at compilation time
note: required by a bound in `Codec::name`
```

Add `where Self: Sized` only to an item no caller needs through `dyn`. If callers need it, move
the item to a second trait, or take `&self` and return an owned type instead of `Self`.

## The three rows the clause does not fix

`where Self: Sized` does not answer the associated const row or the two supertrait rows. A
supertrait bound is not an item, and an associated const rejects the clause: rustc reports
`error[E0658]: generic const items are experimental`. Move an associated const to a second trait.
Drop a `Clone` supertrait and put the clone in the vtable:

```rust
trait Shape {
    fn area(&self) -> f64;
    fn clone_box(&self) -> Box<dyn Shape>;
}

impl Clone for Box<dyn Shape> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
```

## Search terms

rustc renamed this check from "object safety" to "dyn compatibility". A grep of a current build
log for "object safe" finds nothing. Grep for `E0038` or for "dyn compatible".

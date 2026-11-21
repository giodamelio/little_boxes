# `Any` probes

Deep material for the rust-type-erasure skill: the programs behind its `Any`, `type_name`, and
upcast rules. Quoted rustc output comes from rustc 1.98.1, edition 2024, aarch64-apple-darwin.

## The 'static bound fails at the caller

The bound on `put` compiles clean. The first failure is at the call site, and it blames a local:

```rust,compile_fail,E0597
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Default)]
struct AnyMap { table: HashMap<TypeId, Box<dyn Any>> }

impl AnyMap {
    // This definition compiles clean. The bound never fails here.
    fn put<S: Any>(&mut self, x: S) { self.table.insert(TypeId::of::<S>(), Box::new(x)); }
}

fn main() {
    let s = String::from("hello world");
    let mut map = AnyMap::default();
    map.put(Cow::from(&s));         // error[E0597] blames `s`, not `put`
}
```

```text
error[E0597]: `s` does not live long enough
16 |     map.put(Cow::from(&s));
   |     ------------------^^--
   |     |                 borrowed value does not live long enough
   |     argument requires that `s` is borrowed for `'static`
note: requirement that the value outlives `'static` introduced here
10 |     fn put<S: Any>(&mut self, x: S) { ... }
   |               ^^^
```

## Two types share one type_name

As of Rust 1.99.1, `type_name` prints a lifetime argument of a named type as `'_` and omits
reference lifetimes; Rust 1.88 omitted both.

```rust,run
use std::any::type_name;

struct Wrapper<'a>(&'a str);

// Compiles for every `'a`. `type_name` carries no `T: 'static` bound.
fn name_of<'a>(_: &'a str) -> &'static str { type_name::<Wrapper<'a>>() }

fn main() {
    let s = String::from("x");
    assert_eq!(type_name::<Wrapper<'static>>(), name_of(&s));   // "<module>::Wrapper<'_>"
    assert_eq!(type_name::<&'static str>(), type_name::<&str>());   // "&str"
}
```

## Upcast and downcast through a Box

```rust,run
use std::any::{Any, TypeId};

trait Plugin: Any { fn name(&self) -> &'static str; }

struct Metrics;
impl Plugin for Metrics { fn name(&self) -> &'static str { "metrics" } }

fn main() {
    let boxed: Box<dyn Plugin> = Box::new(Metrics);
    // Method lookup stops at the `Box`, which is `'static` and so is `Any` itself.
    assert_eq!(boxed.type_id(), TypeId::of::<Box<dyn Plugin>>());
    // Deref to the trait object, then upcast. This reaches the value.
    let any: &dyn Any = &*boxed;
    assert_eq!(any.type_id(), TypeId::of::<Metrics>());
    assert!(any.downcast_ref::<Metrics>().is_some());
    // An owned upcast keeps the allocation and allows `downcast`.
    let owned: Box<dyn Any> = boxed;
    let metrics: Box<Metrics> = owned.downcast().expect("stored as Metrics");
    assert_eq!(metrics.name(), "metrics");
}
```

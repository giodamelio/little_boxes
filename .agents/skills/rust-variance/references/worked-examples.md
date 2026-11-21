# Worked examples

Deep material for the rust-variance skill: the complete programs and diagnostics behind its
rules. Every result comes from rustc 1.98.1, edition 2024, aarch64-apple-darwin, and Miri 0.1.0
on nightly 2026-05-15.

Contents:

- [Cell fails both probes](#cell-fails-both-probes)
- [An unbounded lifetime outlives its owner](#an-unbounded-lifetime-outlives-its-owner)
- [Lock intended invariance with a doctest pair](#lock-intended-invariance-with-a-doctest-pair)
- [A fn item fails a trait bound](#a-fn-item-fails-a-trait-bound)
- [Three fixes for a producer](#three-fixes-for-a-producer)
- [A higher-ranked callback field](#a-higher-ranked-callback-field)

## Cell fails both probes

```rust,compile_fail
use std::cell::Cell;
fn cov<'a, 'b: 'a>(x: Cell<&'b u8>) -> Cell<&'a u8> { x }
```

```text
error: lifetime may not live long enough
2 | fn cov<'a, 'b: 'a>(x: Cell<&'b u8>) -> Cell<&'a u8> { x }
  = note: requirement occurs because of the type `Cell<&u8>`, which makes the generic argument `&u8` invariant
  = note: the struct `Cell<T>` is invariant over the parameter `T`
```

## An unbounded lifetime outlives its owner

This compiles, reads freed memory at run time with an unpredictable result, and Miri reports
`constructing invalid value of type &std::string::String: encountered a dangling reference (use-after-free)`:

```rust
// UB: `'a` is tied to no input, so `escaped` outlives `owned`.
unsafe fn deref_unbounded<'a, T>(p: *const T) -> &'a T { unsafe { &*p } }

fn main() {
    let escaped: &String;
    {
        let owned = String::from("gone");
        escaped = unsafe { deref_unbounded(&owned as *const String) };
    }
    println!("{escaped}");
}
```

## Lock intended invariance with a doctest pair

The type must be public, in a library target, and each doctest imports it by its crate path,
because a doctest compiles as a separate crate. The first doctest is the compiling twin:

```rust
use std::cell::Cell;

/// ```
/// use my_crate::ConfigV2;
/// fn same<'a>(c: ConfigV2<'a>) -> ConfigV2<'a> { c }
/// ```
///
/// ```compile_fail
/// use my_crate::ConfigV2;
/// fn cov<'a, 'b: 'a>(c: ConfigV2<'b>) -> ConfigV2<'a> { c }
/// ```
pub struct ConfigV2<'a> { pub name: Cell<&'a str> }
```

## A fn item fails a trait bound

```rust,compile_fail,E0597
fn service_name() -> &'static str { "Service" }

fn main() {
    let service = "Service".to_string();
    let mut names: Vec<&str> = vec![&service];
    names.resize_with(10, service_name);
}
```

```text
error[E0597]: `service` does not live long enough
6 |     names.resize_with(10, service_name);
  |     ----------------------------------- argument requires that `service` is borrowed for `'static`
note: requirement that the value outlives `'static` introduced here
    --> .../library/alloc/src/vec/mod.rs
     |         F: FnMut() -> T,
```

## Three fixes for a producer

```rust
fn any_name<'a>() -> &'a str { "Service" }          // fix 1: unbounded output lifetime
fn static_name() -> &'static str { "Service" }

fn main() {
    let service = "Service".to_string();
    let mut names: Vec<&str> = vec![&service];
    names.resize_with(4, any_name);                 // fix 1
    names.resize_with(6, || static_name());         // fix 2: wrap at the call site
    let f: fn() -> &'static str = static_name;      // fix 3: coerce to a fn pointer
    names.resize_with(8, f);
    assert_eq!(names.len(), 8);
}
```

## A higher-ranked callback field

```rust
pub struct Event<'a> { pub name: &'a str }
pub struct Hooks { pub on: Box<dyn for<'x> Fn(&Event<'x>)> }

fn main() {
    let h = Hooks { on: Box::new(|e| { let _ = e.name; }) };
    let name = String::from("tick");
    (h.on)(&Event { name: &name });
}
```

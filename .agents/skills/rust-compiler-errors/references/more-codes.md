# More error codes

Deep material for the rust-compiler-errors skill: the codes its triage table does not hold, and
the never-type fallback programs. The table records rustc 1.98.1 diagnostics.
The fallback programs are checked on Rust 1.100.0.

Contents:

- [Definition, operator, and API codes](#definition-operator-and-api-codes)
- [The never-type fallback](#the-never-type-fallback)

## Definition, operator, and API codes

| Code | Message | What it means | First move |
| --- | --- | --- | --- |
| E0271 | expected `A` to be an iterator that yields `B` | An associated type does not match | Fix the item type, usually with `map` |
| E0369 | cannot add `T` to `T`; binary operation `==` cannot be applied to type `T` | An operator on a generic `T` with no operator bound | Add the bound that `help:` names, such as `T: std::ops::Add<Output = T>` or `T: PartialEq` |
| E0631 | type mismatch in function arguments | A function item was passed to a higher-order call; no deref coercion applies at a trait bound | Wrap the call in a closure, or insert `.map(String::as_str)` |
| E0562 | `impl Trait` is not allowed in the return type of `Fn` trait bounds | `impl FnMut(&T) -> impl Ord` puts `impl Trait` in an associated-type binding | Name a generic parameter instead: `<K: Ord>` |
| E0184 | the trait `Copy` cannot be implemented for this type; the type has a destructor | One type asks for both `Copy` and `Drop` | Remove the `Copy` derive; a resource handle is not `Copy` |
| E0367 | `Drop` impl requires `T: Clone` but the struct it is implemented for does not | The `Drop` impl added a bound the type definition does not carry | Move the bound onto the struct definition |
| E0740 | field must implement `Copy` or be wrapped in `ManuallyDrop<...>` | A union field is not `Copy`, `ManuallyDrop<T>`, a reference, or a tuple or array of those | Derive `Copy` when the type allows it. Otherwise wrap the field in `std::mem::ManuallyDrop` and drop it by hand |
| E0603 | `x` is private | The path exists but is not exported | `pub use` it, or use the public path |
| E0658 | use of unstable library feature `f` | The API is unstable, or stable only in a newer Rust than this toolchain | Compare `rustc -V` and the crate's `rust-version` with the version on the API's docs page |
| E0015 | cannot call non-const function `f` in constants | A `const` or `static` initializer calls a non-`const fn` | Make `f` a `const fn`, or compute the value at run time in a `std::sync::LazyLock` |
| E0072 | recursive type has infinite size | A type contains itself by value | `Box`, `Rc`, or `Arc` the recursive field; `Weak` for a back edge |
| E0793 | reference to field of packed struct is unaligned | A reference into `#[repr(packed)]` | Copy the field out, or `(&raw const f).read_unaligned()` |

## The never-type fallback

An unconstrained generic call as a statement, such as `f()?;`, can rely on never-type fallback.
Rust 1.100 falls back to `!` instead of `()` on every edition, so the error names the never type:

```rust,compile_fail,E0277
fn parse_or_default<T: Default>() -> Result<T, ()> { Ok(T::default()) }

fn load() -> Result<(), ()> {
    parse_or_default()?; // the trait bound `!: Default` is not satisfied
    Ok(())
}
```

The note says "this error might have been caused by changes to Rust's type-inference algorithm".
Before Rust 1.100, edition 2021 reported `dependency_on_unit_never_type_fallback`
(deny by default since 1.92). On 1.100 it reports E0277, like edition 2024.
Name the type on every edition:

```rust
fn parse_or_default<T: Default>() -> Result<T, ()> { Ok(T::default()) }

fn load() -> Result<(), ()> {
    parse_or_default::<()>()?;
    Ok(())
}
```

`let () = parse_or_default()?;` also works.

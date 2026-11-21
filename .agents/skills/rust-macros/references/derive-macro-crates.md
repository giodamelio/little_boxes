# Derive macro crates

Contents:

- [The two-crate split](#the-two-crate-split)
- [The reverse edge is fatal](#the-reverse-edge-is-fatal)
- [The back edge as a dev-dependency is legal](#the-back-edge-as-a-dev-dependency-is-legal)
- [A syn 3 derive skeleton](#a-syn-3-derive-skeleton)
- [Emit absolute paths](#emit-absolute-paths)
- [Bound only the parameters the body uses](#bound-only-the-parameters-the-body-uses)
- [Say which derive is missing](#say-which-derive-is-missing)
- [Checklist](#checklist)

Everything below was measured on a three-crate workspace (`facade`, `facade_derive`, `user`),
rustc 1.98.1 and cargo 1.98.1, edition 2024. `facade` and `facade_derive` are placeholder
names; use your own.

## The two-crate split

A derive macro cannot live beside the trait it implements. A crate with `proc-macro = true`
exports macro functions only, so the trait needs a second, normal crate.

Ship the pair as **one** dependency. The facade crate holds the trait, depends on the derive
crate, and re-exports the derive. The user then adds one dependency and writes one `use`.

```toml
# facade/Cargo.toml
[package]
name = "facade"
version = "0.1.0"
edition = "2024"

[dependencies]
facade_derive = { path = "../facade_derive", version = "=0.1.0" }
```

```toml
# facade_derive/Cargo.toml
[package]
name = "facade_derive"
version = "0.1.0"
edition = "2024"

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
quote = "1"
syn = "3"

# Legal, and it is how this crate tests itself. See "The back edge" below.
[dev-dependencies]
facade = { path = "../facade" }
```

```rust,ignore
// facade/src/lib.rs
pub trait Named {
    fn name() -> &'static str;
}

// A trait and a macro live in different namespaces, so both may be called `Named`.
pub use facade_derive::Named;
```

One import in user code then enables the derive and the trait method:

```rust,ignore
use facade::Named;

#[derive(Named)]
struct Plain;

fn main() {
    println!("{}", Plain::name());
}
```

Lock the two version numbers with an exact requirement, `version = "=0.1.0"`, and release
them together. A bare `version = "0.1.0"` is the caret range `^0.1.0`, so it accepts every
0.1.x derive. A facade that accepts a range of derive versions ships a derive whose output the
trait no longer matches.

## The reverse edge is fatal

The facade already depends on the derive crate. A normal dependency from the derive crate
back to the facade closes the cycle at once. Cargo resolves the package graph from the
manifests alone, so the refusal does not wait for any source change. `cargo build` and
`cargo tree` both fail as soon as the second edge lands:

```text
error: cyclic package dependency: package `facade v0.1.0 (...)` depends on itself. Cycle:
package `facade v0.1.0 (...)`
    ... which satisfies path dependency `facade` (locked to 0.1.0) of package `facade_derive v0.1.0 (...)`
    ... which satisfies path dependency `facade_derive` (locked to 0.1.0) of package `facade v0.1.0 (...)`
```

The `(locked to 0.1.0)` part appears when a `Cargo.lock` already exists. The message names no
source line and no `.rs` file. It appears on `cargo build -p user`, in a crate that touched
neither manifest.

Deleting the re-export does not break the cycle, because cargo reads the manifests and not
the source. It also pushes two dependencies and two version numbers onto every user. Delete
the derive crate's normal dependency on the facade instead, and emit fully qualified paths
from the macro.

## The back edge as a dev-dependency is legal

A `[dev-dependencies]` edge from the derive crate back to the facade does not form a cycle,
because dev-dependencies are excluded from the build graph of the library itself. This is how a
derive crate tests its own output.

```rust,ignore
// facade_derive/tests/derive_test.rs — a separate crate, so the derive is usable here.
use facade::Named;

#[derive(Named)]
struct Inside;

#[test]
fn emits_the_name() {
    assert_eq!(Inside::name(), "Inside");
}
```

Verified: `cargo test -p facade_derive` reports `test emits_the_name ... ok` and
`test result: ok. 1 passed; 0 failed`.

## A syn 3 derive skeleton

Keep the entry point thin. A `#[proc_macro_derive]` function compiles only in a
`proc-macro = true` crate:

```rust,ignore
// facade_derive/src/lib.rs
#[proc_macro_derive(Checked, attributes(checked))]
pub fn derive_checked(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand(input).unwrap_or_else(syn::Error::into_compile_error).into()
}
```

Put the logic in a function on `proc_macro2` types. It reports a rejected shape as a
`syn::Error` at a precise span, and it splits the generics so the impl carries the type's own
parameters and where-clause:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error};

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(_) = &input.data else {
        return Err(Error::new_spanned(&input.ident, "Checked supports structs only"));
    };
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::facade::Checked for #name #ty_generics #where_clause {}
    })
}
```

`split_for_impl` copies every bound the user wrote on the type. It adds no `T: Checked` bound;
add one only where the generated body needs it (see
[Bound only the parameters the body uses](#bound-only-the-parameters-the-body-uses)).

A panic reaches the user with the span of the whole derive and no hint about the field:

```text
error: proc-macro derive panicked
 --> user/src/main.rs:1:10
  |
1 | #[derive(::facade::Boom)]
  |          ^^^^^^^^^^^^^^
  |
  = help: message: unsupported shape
```

The `syn::Error` from `expand` points at the name it was given instead:

```text
error: Checked supports structs only
 --> user/src/main.rs:7:6
  |
7 | enum Bad {
  |      ^^^
```

## Emit absolute paths

A derive expands in a crate you do not control. A relative path such as `facade::Named` is
resolved in the user's module, so any local item named `facade` captures it. The leading `::`
forces the extern-prelude crate:

```rust
let name = quote::format_ident!("Rel");

// Rejected: `facade::Named` resolves in the caller's module.
let relative = quote::quote! { impl facade::Named for #name {} };

// Correct: `::facade` can only name the dependency.
let absolute = quote::quote! { impl ::facade::Named for #name {} };
```

The failure needs only a module with the dependency's name:

```rust,ignore
mod shadow {
    pub mod facade {}          // an unrelated local module

    use ::facade::Named;

    #[derive(Named)]
    pub struct Rel;
}
```

```text
error[E0405]: cannot find trait `Named` in module `facade`
 --> user/src/main.rs:4:14
  |
4 |     #[derive(Named)]
  |              ^^^^^ not found in `facade`
  |
  = note: this error originates in the derive macro `Named`
```

The span points at the `#[derive(...)]` line in the user's file, not at the macro source. The
identical derive that emits `::facade::Named` compiles and runs in the same module.

`::facade::...` is correct only while the dependency keeps that crate name. When a user may
rename it in `Cargo.toml`, read the path from a helper attribute and default to `::facade`:

```rust,ignore
#[derive(Named)]
#[named(crate = "::renamed_facade")]
struct Custom;
```

Declare the attribute, or the user's build fails at the attribute rather than at the derive:

```rust,ignore
#[proc_macro_derive(Named, attributes(named))]
pub fn derive_named(input: TokenStream) -> TokenStream {
    todo!()
}
```

```text
error: cannot find attribute `named` in this scope
```

## Bound only the parameters the body uses

The common `add_trait_bounds` helper pushes the trait bound onto every type parameter of the
input. That is wrong whenever a parameter does not appear in the generated body. The classic
case is `PhantomData`.

```rust,compile_fail,E0277
use std::marker::PhantomData;

pub trait Named {
    fn name() -> &'static str;
}

pub struct Tag<T> {
    _m: PhantomData<T>,
}

// The blanket bound a naive derive emits.
impl<T: Named> Named for Tag<T> {
    fn name() -> &'static str {
        "Tag"
    }
}

fn main() {
    println!("{}", <Tag<u32> as Named>::name());
}
```

```text
error[E0277]: the trait bound `u32: Named` is not satisfied
   |
help: the trait `Named` is implemented for `Tag<T>`
note: required for `Tag<u32>` to implement `Named`
```

Drop the bound when the body does not call through the parameter:

```rust
use std::marker::PhantomData;

pub trait Named {
    fn name() -> &'static str;
}

pub struct Tag<T> {
    _m: PhantomData<T>,
}

// No bound on `T`: the body never uses it.
impl<T> Named for Tag<T> {
    fn name() -> &'static str {
        "Tag"
    }
}

fn main() {
    println!("{}", <Tag<u32> as Named>::name());
}
```

Decide the bound per field, not per parameter:

| The generated body | Bound to emit |
| --- | --- |
| Calls the trait on a field of type `T` | `T: Trait` |
| Calls the trait on a field of type `Vec<T>` or `Option<T>` | The field type, as a where-clause: `Vec<T>: Trait`. It holds whatever the container's impl requires. `T: Trait` is right only when that impl is `impl<T: Trait> Trait for Vec<T>`, and it is wrong when the impl has another bound or none |
| Never names `T` at run time (`PhantomData<T>`, a skipped field) | No bound |
| Calls the trait on an associated type `T::Assoc` | `T::Assoc: Trait`, as a where-clause |

Both defects — the relative path and the blanket bound — compile in your own workspace and fail
only in a downstream crate. Add one integration test per shape you support, in the derive
crate's `tests/` directory.

## Say which derive is missing

A user who forgets the derive gets a plain E0277 at the first generic call. Put
`#[diagnostic::on_unimplemented]` (Rust 1.78+) on the facade trait to name the fix:

```rust,compile_fail,E0277
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `Named`",
    label = "missing `#[derive(Named)]`",
    note = "add `#[derive(facade::Named)]` to the definition of `{Self}`"
)]
pub trait Named {
    fn name() -> &'static str;
}

fn print_name<T: Named>() {
    println!("{}", T::name());
}

struct Plain;

fn main() {
    print_name::<Plain>();
}
```

```text
error[E0277]: `Plain` does not implement `Named`
   |
17 |     print_name::<Plain>();
   |                  ^^^^^ missing `#[derive(Named)]`
   = note: add `#[derive(facade::Named)]` to the definition of `Plain`
```

## Checklist

- The facade crate depends on the derive crate. The derive crate has no normal dependency on the facade.
- The facade re-exports the derive, so the user adds one dependency.
- The version requirement between the two crates is exact, and both crates release together.
- The derive crate tests itself from `tests/`, through a `[dev-dependencies]` edge.
- The entry point only parses and maps a `syn::Error` to `into_compile_error`; no code path panics.
- Every path in the emitted tokens starts with `::`.
- A renameable dependency path comes from a helper attribute, and the attribute is declared in `attributes(...)`.
- Bounds are emitted per field, not per type parameter.
- A `PhantomData<T>` case and a generic case both appear in the tests.
- The facade trait carries `#[diagnostic::on_unimplemented]` that names the derive.

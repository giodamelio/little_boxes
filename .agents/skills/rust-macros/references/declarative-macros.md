# Declarative macro details

Contents:

- [Textual scope](#textual-scope)
- [Scope a macro by path](#scope-a-macro-by-path)
- [Edition 2024 widened `expr`](#edition-2024-widened-expr)
- [The recursion limit is 128 expansions](#the-recursion-limit-is-128-expansions)
- [Absolute path details](#absolute-path-details)
- [A caller's alias shadows a bare standard-library name](#a-callers-alias-shadows-a-bare-standard-library-name)
- [Hygiene examples](#hygiene-examples)
- [Follow sets](#follow-sets)
- [Build a format string](#build-a-format-string)
- [Nightly macro syntax](#nightly-macro-syntax)

Every quoted message comes from rustc 1.99.1, edition 2024.

## Textual scope

A refactor that moves the definition down the file breaks the build, and the
message names no missing import.

```text
error: cannot find macro `hello` in this scope
  |             ^^^^^ consider moving the definition of `hello` before this call
note: a macro with the same name exists, but it appears later
```

The same rule applies across modules. A macro defined in `mod a` is invisible in `mod b`, even
when `b` comes later:

```text
error: cannot find macro `only_here` in this scope
  = help: have you added the `#[macro_use]` on the module/import?
```

Four ways to widen the scope; prefer the first, which keeps the macro private.

| Goal | Write | Effect |
| --- | --- | --- |
| Use it in another module, crate-private | `pub(crate) use name;` after the definition | The macro becomes an ordinary item of that module, reachable by path |
| Use it in later modules of the same crate | `#[macro_use] mod a;` | Later code in the parent module sees it |
| Import it by path anywhere in the crate | `#[macro_export]` on the definition, then `use crate::name;` | The macro lands at the crate root, whatever module defines it |
| Export it to other crates | `#[macro_export]`, then `use thatcrate::name;` in the caller | Same crate-root placement, now public |

## Scope a macro by path

`pub(crate) use` after the definition makes the macro an ordinary crate-private item, reachable
by path from any module:

```rust
mod math {
    macro_rules! twice {
        ($v:expr) => {
            $v * 2
        };
    }
    pub(crate) use twice;
}

fn main() {
    println!("{}", math::twice!(21));
}
```

## Edition 2024 widened `expr`

In edition 2024 `$e:expr` also matches `const { 1 + 1 }` and the underscore expression `_`.
Edition 2021 rejects the const block with ``error: no rules expected keyword `const` `` and
the underscore with ``error: no rules expected reserved identifier `_` ``. An arm that
assumes the capture is a value expression can now receive `_`, which is not one. Use
`expr_2021` to keep the edition 2021 match set.

## The recursion limit is 128 expansions

A `tt`-muncher costs one expansion per token. Measured with a counting muncher: 127 input
tokens compile, 128 fail with ``error: recursion limit reached while expanding `count!` ``.
The limit of the crate that invokes the macro applies. A `#![recursion_limit = "256"]` in the
macro's own crate does not help a downstream caller, so an exported macro must stay under 128
expansions per call. Prefer a shape that halves the input per step.

## Absolute path details

The full table behind the `SKILL.md` rule:

| The path names | Emit | Because |
| --- | --- | --- |
| An item of the macro's own crate | `$crate::render(...)` | A bare `render` fails in every other crate with E0425. `crate::render` names the caller's crate, and `clippy::crate_in_macro_def` flags it |
| A standard-library item | `::core::result::Result`. For an `alloc` item, put `pub use alloc::vec::Vec;` in `$crate::__private` and emit `$crate::__private::Vec`. Emit `::std::` only for an item that neither `core` nor `alloc` has, and only when no `#![no_std]` caller is supported. Do not emit `::alloc::` | A bare `Result` resolves to the caller's `type Result<T> = ...` alias. `::std::` fails with E0433 in a `#![no_std]` caller. `::alloc::` fails with E0433 in every caller that does not write `extern crate alloc;`, ordinary std crates included, because `alloc` is not in their extern prelude (verified on Rust 1.99.1) |
| An item of a dependency of the macro crate | Re-export it (`#[doc(hidden)] pub mod __private { pub use dep_name; }`) and emit `$crate::__private::dep_name::Item`. From a derive, re-export it from the facade and emit `::facade::__private::dep_name::Item` | `::dep_name` resolves in the caller's crate, and the caller may not depend on `dep_name` |
| The derive attribute | `#[::core::derive(...)]` when the macro crate's `rust-version` is 1.96 or later, else `#[::core::prelude::v1::derive(...)]` | `derive` also resolves at the call site. Rust 1.95 rejects `::core::derive` with E0433 |

A missing `$crate` passes in one place only: a module of the defining crate that already has the
item in scope. It fails in every downstream crate, in a `#[cfg(test)] mod tests` that does not
`use super::*`, and in your own `tests/*.rs`. `::std::` fails with E0433 in a `#![no_std]`
caller. `::alloc::` fails in ordinary std crates too, because `alloc` is not in their extern
prelude (verified on Rust 1.99.1). A `tests/*.rs` invocation does not catch a `::dep_name::` path, because
an integration test sees every dependency of its package.

## A caller's alias shadows a bare standard-library name

A macro expands in the caller's module, so a bare `Result` in the macro body resolves to the
caller's `type Result<T> = ...` alias:

```rust,compile_fail,E0107
macro_rules! parse_fn {
    ($name:ident) => {
        pub fn $name(_s: &str) -> Result<i32, String> {
            todo!()
        }
    };
}

mod caller {
    type Result<T> = core::result::Result<T, std::io::Error>;
    parse_fn!(parse);
}

fn main() {}
```

The error is `type alias takes 1 generic argument but 2 generic arguments were supplied`.
`::core::result::Result<i32, ::std::string::String>` in the macro body fixes it for a caller that
links `std`.

## Hygiene examples

A generated local is invisible outside the expansion. rustc adds `help: an identifier with the
same name is defined here, but is not accessible due to macro hygiene`:

```rust,compile_fail,E0425
macro_rules! decl {
    () => {
        let tmp = 7;
    };
}

fn main() {
    decl!();
    println!("{tmp}");
}
```

A generated item is visible, so a fixed item name collides on the second invocation with
``error[E0428]: the name `Inner` is defined multiple times``:

```rust,compile_fail,E0428
macro_rules! mk_tag {
    ($n:ident) => {
        struct $n;
        struct Inner;
    };
}

mk_tag!(Alpha);
mk_tag!(Beta);

fn main() {}
```

The anonymous `const _: () = { ... };` fix. Each expansion of the block gets its own item
namespace, and an `impl` inside it still applies to the outer type:

```rust
macro_rules! mk_tag {
    ($n:ident) => {
        pub struct $n;

        const _: () = {
            struct Guard;
            impl $n {
                pub fn tag() -> &'static str {
                    let _g = Guard;
                    stringify!($n)
                }
            }
        };
    };
}

mk_tag!(Alpha);
mk_tag!(Beta);

fn main() {
    println!("{} {}", Alpha::tag(), Beta::tag());
}
```

## Follow sets

rustc checks these rules when it reads the `macro_rules!` item. The rules are in the
[Reference](https://doc.rust-lang.org/reference/macros-by-example.html#follow-set-ambiguity-restrictions):

| Fragment | Tokens allowed after it |
| --- | --- |
| `expr`, `stmt` | `=>`, `,`, `;` |
| `pat` | `=>`, `,`, `=`, `if`, `in` |
| `pat_param` | The `pat` set, plus `\|` |
| `ty`, `path` | `{`, `[`, `=>`, `,`, `>`, `>>`, `=`, `:`, `;`, `\|`, `as`, `where`, a `$b:block` metavariable |
| `vis` | `,`, an identifier other than `priv`, any token that starts a type, an `ident`, `ty`, or `path` metavariable |
| `ident`, `lifetime`, `literal`, `block`, `meta`, `tt`, `item` | No restriction |

```text
error: `$e:expr` is followed by `$s:stmt`, which is not allowed for `expr` fragments
  = note: allowed there are: `=>`, `,` or `;`
```

A captured fragment is opaque to the next matcher:

```text
error: no rules expected `expr` metavariable
note: while trying to match meta-variable `$i:ident`
```

The note for `ty` and `path` omits `>>` and a `$b:block` metavariable. Both compile.

`pat` matches a top-level or-pattern since edition 2021, which is why `|` cannot follow it. Use
`pat_param` when `|` must be your separator.

## Build a format string

A format string must be a literal token. A variable or a runtime `String` fails with
`error: format argument must be a string literal`.

Three routes work. Forward the caller's literal with `$fmt:literal` (or `$fmt:tt`). Prefix it
with `concat!("prefix", $fmt)`. Build one from identifiers with `concat!` over `stringify!`.

```rust,run
macro_rules! log {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        println!(concat!("[app] ", $fmt) $(, $arg)*)
    };
}

macro_rules! dump {
    ($($v:ident),* $(,)?) => {
        println!(concat!($(stringify!($v), "={:?} "),*), $($v),*)
    };
}

fn main() {
    let count = 7;
    let name = "x";
    log!("count is {}", count);
    dump!(count, name);
}
```

A format string that comes from `concat!` cannot capture variables inline. `log!("{count}")`
fails with ``error: there is no argument named `count` ``, and the note says `format_args!`
cannot capture variables when the format string is expanded from a macro. Pass the arguments
explicitly, or forward `$fmt` to `println!` unchanged when you need inline capture.

## Nightly macro syntax

As of Rust 1.99.1, these forms fail on the stable toolchain. Do not copy them from nightly code.

| Form | Error |
| --- | --- |
| A `macro_rules!` arm written `attr() (...) => ...` | ``error[E0658]: `macro_rules!` attributes are unstable`` |
| A `macro_rules!` arm written `derive() (...) => ...` | ``error[E0658]: `macro_rules!` derives are unstable`` |
| `${count($x)}` | `error[E0658]: meta-variable expressions are unstable` |

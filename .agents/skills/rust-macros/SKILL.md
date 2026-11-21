---
description: Use when writing, debugging, or reviewing a Rust macro_rules! macro, a procedural macro (derive, attribute, or function-like) built on syn and quote, its $crate paths and trybuild compile-fail tests, or its facade and derive crate pair. Triggers on "cannot find macro in this scope", "recursion limit reached while expanding", "proc-macro derive panicked", "trailing semicolon in macro used in expression position", "cyclic package dependency", "write a derive macro", "proc macro", "macro hygiene", "fragment specifier", "syn 3", "cargo expand".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-macros
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: eb6cb70e389e66daef9e10c06674075fd8a75452
name: rust-macros
---
# Rust macros

Every quoted message comes from rustc, cargo, and Clippy 1.99.1, edition 2024.

## Decide whether you need a macro

| You want | Reach for |
| --- | --- |
| One body, many types | A generic function, or a blanket impl |
| A constant computed at build time | `const fn`, or a `const { }` block |
| Code generated from a schema or a data file | `build.rs` plus `include!` |
| Items or an expression chosen per target or feature | `cfg_select!` (std, Rust 1.95+), not `cfg-if` |
| Repeated syntax inside one crate | `macro_rules!` |
| The same trait impl per type, derived from its shape | A derive macro |
| An item rewritten or wrapped | An attribute macro (procedural) |

A macro costs compile time, a second crate when procedural, diagnostics at the invocation, and
an IDE that cannot look inside. Take a non-macro row when one fits.

Do not copy nightly macro syntax: `attr()` and `derive()` arms and `${count($x)}` fail with
E0658 on stable ([messages](references/declarative-macros.md#nightly-macro-syntax)).

## Route the symptom

| Symptom or task | Section |
| --- | --- |
| `error: cannot find macro ... in this scope` | [Scope is textual](#scope-is-textual) |
| E0425 or E0433 in another crate from an exported macro, or `this error originates in the macro` on a caller's type | [Emit absolute paths](#emit-absolute-paths) |
| E0425 with a macro-hygiene note, or E0428 from two invocations | [Hygiene stops at bindings](#hygiene-stops-at-bindings) |
| ``$x:frag` is followed by ...``, `missing fragment specifier`, `no rules expected` when forwarding, or `recursion limit reached while expanding` | [Fragment specifiers](#fragment-specifiers) |
| `error: trailing semicolon in macro used in expression position` | [Trailing semicolons](#a-trailing-semicolon-breaks-expression-position) |
| `format argument must be a string literal`, or `there is no argument named` | [Build a format string](#build-a-format-string) |
| A proc-macro crate error, a derive panic, or an undeclared helper attribute | [Procedural macro crates](#procedural-macro-crates) |
| Choosing a `syn` version, or E0599/E0609 on `BareFn`, `unsafety`, or `guard` | [Pick the syn major](#pick-the-syn-major) |
| `cyclic package dependency` from a derive-crate back edge, or E0405 from a derive | [references/derive-macro-crates.md](references/derive-macro-crates.md) |
| See the expansion, or prove the macro works | [Read the expansion and verify](#read-the-expansion-and-verify) |

## Read the expansion and verify

Read the expansion before you reason about a failure.

```bash
cargo +nightly rustc --locked -p <crate> --profile=check -- -Zunpretty=expanded
# On stable: cargo-expand sets RUSTC_BOOTSTRAP=1 for the -Zunpretty call.
cargo install --locked cargo-expand
cargo expand --locked --package <crate> <path::to::item>
```

The text expansion prints hygienic identifiers as plain names. Do not paste it back as source.

| Claim | Check |
| --- | --- |
| The expansion works in another crate | An invocation in `tests/*.rs`: `cargo test --locked -p <macro-crate>` |
| Emitted paths survive the caller's names | A test module with `mod facade {}` and `type Result<T> = ...;` that invokes the macro |
| A rejected input gives the intended message | `trybuild` compile-fail cases, same command |
| No `crate::` path, no metavariable inside `unsafe` | `cargo clippy --locked --all-targets -- -D warnings` (`crate_in_macro_def`, `macro_metavars_in_unsafe` with its [preconditions](#keep-caller-expressions-out-of-unsafe-blocks)) |
| No trailing `;` in an expression-position arm | One expression-position invocation inside the defining crate |
| The facade works | `cargo test --locked -p <facade-crate>` |

rustc skips most lints, `unsafe_code` included, in another crate's expansion: lint through an
invocation inside the defining crate.

Read [references/review-checklist.md](references/review-checklist.md#set-up-trybuild) when you
set up `trybuild`: run it on one pinned toolchain only, and review every `.stderr` it writes.

A green run proves only the shapes you tested: add a generic type, a `PhantomData<T>` field, a
unit struct, and each rejected shape. Review a finished change against the same checklist.

## Emit absolute paths

A macro expands in the caller's module. Every path in its output resolves there, against the
caller's imports and local items.

| The path names | Emit | Because |
| --- | --- | --- |
| The macro's own crate | `$crate::render(...)` | A bare name is E0425 elsewhere; `crate::` names the caller's crate |
| `std` | `::core::result::Result`; an `alloc` item via `$crate::__private::Vec`; `::std::` only with no `#![no_std]` caller. Never `::alloc::` | A bare `Result` meets the caller's alias; `::alloc::` is E0433 without `extern crate alloc;` |
| A dependency | `$crate::__private::dep_name::Item`, re-exported from a `#[doc(hidden)] pub mod __private`; from a derive, `::facade::__private::...` | The caller may not depend on it |
| `derive` | `#[::core::derive(...)]` with `rust-version` 1.96+, else `#[::core::prelude::v1::derive(...)]` | 1.95 rejects `::core::derive` |

Emit `::dep_name::` only for a crate the caller depends on directly, such as the facade. Never put `$crate::` before a path the caller passed
as a metavariable. A missing `$crate` passes only where the item is already in scope. Read [references/declarative-macros.md](references/declarative-macros.md)#absolute-path-details) when you need the
details or the reproduction. A derive follows the same rule with `::facade::Trait`; see
[references/derive-macro-crates.md](references/derive-macro-crates.md).

## Keep caller expressions out of `unsafe` blocks

`unsafe { f($e) }` runs the caller's `$e` inside your block with no `unsafe` at the call site.
Bind first: `let v = $e;`, then `unsafe { f(v) }`. When soundness depends on the value, emit a
call to an `unsafe fn` outside any block. `clippy::macro_metavars_in_unsafe` fires only on an
invocation in the defining crate, and on private macros only with
`warn-unsafe-macro-metavars-in-private-macros = true` in `clippy.toml`. A downstream
`#![forbid(unsafe_code)]` does not reject your expansion: the macro crate owns its soundness.

## Scope is textual

A `macro_rules!` definition must come **before** its use, and a macro in `mod a` is invisible in
`mod b`. Default: `pub(crate) use name;` after the definition, and call it by path. Do not use
`#[macro_export]` inside one crate: it makes public API. Read
[references/declarative-macros.md](references/declarative-macros.md#textual-scope) for the
messages and other ways to widen the scope.

## Hygiene stops at bindings

Hygiene covers only `let` bindings, parameters, and labels: the caller cannot name them (E0425,
`not accessible due to macro hygiene`). It does **not** cover lifetimes (a generated `<'a>` is the
caller's `'a`; name it from a metavariable), item, type, field, method, or macro names: two invocations collide with E0428.

Derive every generated item name from a metavariable, or wrap the private helpers in an anonymous
`const _: () = { ... };` block, which gives each expansion its own item namespace; an `impl`
inside it still applies to the outer type. Read
[references/declarative-macros.md](references/declarative-macros.md#hygiene-examples) for the
reproductions.

## Fragment specifiers

### Follow sets are checked at definition time

rustc rejects an illegal separator when it reads the `macro_rules!` item. Only `=>`, `,`, or `;`
may follow `expr` or `stmt`, and `|` cannot follow `pat` (use `pat_param`). Read
[references/declarative-macros.md](references/declarative-macros.md#follow-sets) when you pick a
separator after `pat`, `ty`, `path`, or `vis`. `($x) =>` without a fragment specifier fails with
`missing fragment specifier` since Rust 1.89.

### A parsed fragment is opaque to the next matcher

A captured `$e:expr` is one AST node, so a `$i:ident` matcher rejects it (`no rules expected`).
Capture as `tt` in the outer macro, and parse only at the last step.

### Edition 2024 and the recursion limit

In edition 2024 `$e:expr` also matches `const { .. }` and `_`; `expr_2021` keeps the old set. An
exported `tt`-muncher stays under 128 expansions per call, the caller's `recursion_limit`. Read
[references/declarative-macros.md](references/declarative-macros.md#edition-2024-widened-expr)
when either bites.

### A trailing semicolon breaks expression position

The braces around an arm body are delimiters, not a block: `() => { 42; }` expands to the
statement `42;`. `semicolon_in_expressions_from_macros` (deny since 1.91) catches it only in the
defining crate; for a downstream caller, Rust 1.99 adds the warn-by-default
`semicolon_in_expressions_from_non_local_macros`. Remove the `;` after the last expression; for
statements, use double braces: `() => {{ let base = 40; base + 2 }}`.

## Build a format string

A format string must be a literal token: forward `$fmt:literal`, or build one with `concat!`,
which cannot capture variables inline. End every multi-line repetition with `$(,)?`. Read
[references/declarative-macros.md](references/declarative-macros.md#build-a-format-string) when a
macro builds or forwards a format string.

## Procedural macro crates

A procedural macro lives in its own crate:

```toml
[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
quote = "1"
syn = "3"
```

It exports macro functions only and cannot invoke its own macros.

| Symptom | Cause | Fix |
| --- | --- | --- |
| ``error: `proc-macro` crate types currently cannot export any items other than functions ...`` | A `pub` item in the macro crate | Keep helpers private; share types via a support crate, never the facade |
| `error: can't use a procedural macro from the same crate that defines it` | A `#[cfg(test)] mod tests` invokes the macro | Put macro tests in `tests/` |
| `error: proc-macro derive panicked` | A `panic!`, `unwrap`, or `expect` in the macro | Return a `syn::Error` at the offending span |
| `cannot find attribute ... in this scope` on a field | An undeclared helper attribute | `#[proc_macro_derive(Checked, attributes(checked))]` |
| ``error[E0659]: `inline` is ambiguous``, or `ambiguous_derive_helpers` | A helper named like a built-in attribute | Name the helper after the derive |

Keep the `#[proc_macro_derive]` function thin: parse with `syn::parse_macro_input!`, put the logic
in `fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream>`, and map an error with
`.unwrap_or_else(syn::Error::into_compile_error)`.

Read [references/derive-macro-crates.md](references/derive-macro-crates.md) when you write a
derive or set up the facade and derive pair.

### Pick the syn major

Use the syn major that `cargo tree --locked -e normal,build -i syn` already shows; with both or
none, use `syn = "3"`. Read [references/syn-3.md](references/syn-3.md) when you choose the major,
port a macro crate, or fix E0599, E0609, or E0432 on a renamed syn item.

## Related skills

When installed: `rust-unsafe` owns the soundness of emitted `unsafe` code, `rust-serde` owns
`#[derive(Serialize, Deserialize)]` attributes, and `rust-test-tools` owns golden tests
(`cargo insta review`). The `rust-lints` skill owns lint policy in `[workspace.lints]`.

# Port a macro crate to syn 3

syn 3.0.0 was released on 2026-07-18. The checked release is 3.0.10 as of 2026-11-13, and its
`rust-version` is still 1.71 ([crates.io](https://crates.io/crates/syn/3.0.10),
[release notes](https://github.com/dtolnay/syn/releases/tag/3.0.0)). `quote 1` and
`proc-macro2 1` work with both majors, so only `syn` changes in `Cargo.toml`.

Version 3.0.10 avoids repeated scans when it prints nested expressions. The public syn 3
names below are unchanged. Keep expansion tests for expression precedence and grouping;
a successful parse alone does not check the tokens that the macro emits.

## Pick the syn major

As of 2026-09, `serde_derive`, `thiserror-impl`,
`tokio-macros`, and `clap_derive` require `syn ^3`, while other popular macro crates still use
syn 2. Each major in the tree is a separate build of a large crate.

```bash
cargo tree --locked -e normal,build -i syn
```

One version prints its dependents. Two majors print ``error: specification `syn` is ambiguous``
and list both. Run `cargo tree --locked -e normal,build -i syn@2` to list the dependencies
that still build syn 2. Use the major the tree already builds. When the tree has both or none, use
`syn = "3"`. syn-2 code fails on syn 3 with E0599, E0609, or E0432 on renamed items such as
`Type::BareFn`, `Signature::unsafety`, and `Arm::guard`: the next sections list each rename.

## Renamed items

Each row was compiled against syn 3.0.6 on rustc 1.98.1. The last column is the error that
syn-2 code produces.

| syn 2 | syn 3 | Error on syn-2 code |
| --- | --- | --- |
| `Type::BareFn(TypeBareFn)` | `Type::FnPtr(TypeFnPtr)` | E0599 no variant `BareFn`; E0432 on `use syn::TypeBareFn` |
| `BareFnArg`, `BareVariadic` | `NamedArg`, `FnPtrVariadic` | E0432 |
| `Signature::unsafety: Option<Token![unsafe]>` | `Signature::safety: Safety` (`Safe`, `Unsafe`, `Default`) | E0609 no field `unsafety` |
| `Arm::guard` | `Pat::Guard(PatGuard)` inside `Arm::pat` | E0609 no field `guard` |
| `TypePtr::const_token` and `TypePtr::mutability` | `TypePtr::mutability: PointerMutability` (`Const`, `Mut`) | E0609 no field `const_token` |
| `ExprClosure::or1_token`, `or2_token` | `inputs_begin`, `inputs_end` | E0609 |
| `Receiver::reference` and related fields | `Receiver::kind: ReceiverKind` | E0609 no field `reference` |
| `Punctuated::pop() -> Option<Pair<T, P>>` | `pop() -> Option<T>`; `pop_pair()` returns the pair | E0599 no method `into_value` |
| `LitInt::from(proc_macro2::Literal)`, same for `LitFloat` | Removed | E0308 |

The release notes list more changes that break a struct literal or an exhaustive match: every
`Type` variant now holds `attrs`, ten new non-exhaustive `*Modifiers` structs, and
`Lifetime::parse` rejects keyword lifetimes. Read the release notes when an error names an item
that is not in the table.

## The syn 3 names in use

This block type-checks against syn 3 with the `full` feature. An arm written
`Some(x) if x > 0 => 1` parses into `Pat::Guard`.

```rust
use syn::punctuated::Punctuated;
use syn::{Arm, Pat, PointerMutability, Safety, Signature, Token, Type, TypePtr};

fn is_fn_pointer(ty: &Type) -> bool {
    matches!(ty, Type::FnPtr(_))
}

fn is_unsafe(sig: &Signature) -> bool {
    matches!(sig.safety, Safety::Unsafe(_))
}

fn has_guard(arm: &Arm) -> bool {
    matches!(arm.pat, Pat::Guard(_))
}

fn is_const_pointer(ptr: &TypePtr) -> bool {
    matches!(ptr.mutability, PointerMutability::Const(_))
}

fn last_type(list: &mut Punctuated<Type, Token![,]>) -> Option<Type> {
    list.pop()
}
```

## Port steps

1. Run `cargo tree --locked -e normal,build -i syn` and record which majors the tree builds.
2. Change `syn = "2"` to `syn = "3"` in the macro crate and keep the same feature list.
3. Resolve the lock without a build: `cargo metadata --format-version 1 > /dev/null`. Read the
   `Cargo.lock` diff, vet every new package name (the `rust-security` skill gate, when it is
   installed), and keep the lock diff in the same change as the manifest edit. Do not run a build
   without `--locked` to update the lock: that compiles and runs unvetted build scripts and proc
   macros.
4. Run `cargo check --locked -p <macro-crate>` and fix each error with the table above.
5. Run `cargo test --locked -p <macro-crate>` so the `tests/` and `trybuild` cases prove the
   expansion.
6. Run `cargo tree --locked -e normal,build -i syn` again. The port removes a build only when no
   other dependency still needs syn 2. When both majors remain, run
   `cargo tree --locked -e normal,build -i syn@2` to list the dependencies that still build syn 2.

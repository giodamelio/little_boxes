# Macro review checklist

Each line restates a rule from `SKILL.md`. Use it to review a finished macro change.

- A macro reached by path (`pub(crate) use name;` after the definition) needs no order. A macro reached only by textual scope (no `use`, no `#[macro_export]`) is defined above every use in its file, and its module's `mod` line comes before the modules that use it.
- `#[macro_export]` appears only on a macro that is part of the public API.
- Every emitted path starts with `$crate::`, `::core::`, or, from a derive, `::facade::`. `::alloc::` never appears; an `alloc` item goes through `$crate::__private`. `::std::` appears only when no `#![no_std]` caller is supported.
- A dependency of the macro crate is reached through a `$crate::__private` re-export, not through `::dep_name::`.
- Every generated item name comes from a metavariable, or sits in an anonymous `const _: () = { ... };`.
- No metavariable expands inside an `unsafe` block.
- Separators respect the follow set of the fragment before them.
- No arm used in expression position ends its body with `;`.
- Every repetition that spans lines ends with `$(,)?`.
- The procedural macro crate uses the syn major the tree already builds, or syn 3.
- A rejected input returns a `syn::Error`, and no code path panics.
- Every helper attribute appears in `attributes(...)`, and none shares a built-in attribute name.
- The author reads the expansion once with `-Zunpretty=expanded` or `cargo expand`.
- Each claim in the verification table of `SKILL.md` has a passing check.

## Set up trybuild

Add `trybuild = "1"` under `[dev-dependencies]` and a `#[test]` that calls
`trybuild::TestCases::new().compile_fail("tests/ui/*.rs")`. Run
`TRYBUILD=overwrite cargo test --locked` to write the `.stderr` files, and review each one before
you commit it. They hold rustc's rendered diagnostics, which change between releases. Run the
`trybuild` test on one pinned toolchain only, and rerun `TRYBUILD=overwrite` on each bump.

A proc-macro test build that invokes the macro from `src/lib.rs` adds
``help: you can define integration tests in a directory named `tests` ``. A derive panic shows
`help: message: ...` with a span on the whole derive.

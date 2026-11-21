# What `#![forbid(unsafe_code)]` catches

The rule and the governance steps are in [SKILL.md](../SKILL.md). This file lists what the lint
reports for each source of `unsafe`, and why a macro from a dependency escapes it.

## Governance detail

A written rule such as "no unsafe in this crate" is not enforcement. Enforce it with
`#![forbid(unsafe_code)]` at the crate root, a lint level, or a CI check. Add the attribute to
every crate that has no hand-written `unsafe`, including a crate that reaches C or the OS only
through a safe wrapper crate. A local `#[allow(unsafe_code)]` cannot weaken it (E0453). The lint
also rejects `#[unsafe(no_mangle)]`, `#[unsafe(export_name)]`, and `#[unsafe(link_section)]`, so a
crate with an unmangled export cannot carry it. Before 1.98 it ignores other unsafe attributes,
such as `#[unsafe(naked)]`.

The lint checks the crate's own source, not the code it compiles to. An unsafe block or an
`#[unsafe(no_mangle)]` export that a dependency's macro expands builds with no diagnostic, and
`cargo clippy -- -D unsafe_code` is silent too. So a `forbid` hit does not prove that the crate
compiles to no unsafe. The table below gives the diagnostic per source of unsafe.

Removing `#![forbid(unsafe_code)]` is a reviewable event. Remove it in the commit that adds the
unsafe code, not in advance, and state which unsafe operation forced it and where it lives. Keep
the set of unsafe crates small, for example only the FFI adapter.

In a periodic audit, also grep the macro expansion of each crate, because `forbid(unsafe_code)`
does not see dependency macro code; the nightly command is below.

## Result by source of `unsafe`

The lint checks the crate's own source text. It does not check the code the crate compiles to.
Re-measured on rustc 1.99.1, edition 2024, for every row. The 1.98 and older attribute results
were measured on 1.98.1, 1.97.1, and 1.88.0:

| Where the `unsafe` comes from | Result under `#![forbid(unsafe_code)]` |
| --- | --- |
| A hand-written `unsafe { ... }` block, or an `unsafe extern` block | ``error: usage of an `unsafe` block``, or ``usage of an `unsafe extern` block``. The build fails. |
| `#[unsafe(no_mangle)]`, `#[unsafe(export_name)]`, or `#[unsafe(link_section)]` in the crate | One error per attribute on every edition-2024 toolchain. Since 1.99: ``usage of the unsafe `no_mangle` attribute``. On 1.98: ``usage of the unsafe `#[no_mangle]` attribute``. Before 1.98: ``declaration of a `no_mangle` function``, and the same `declaration of ...` form for the other two. |
| Other unsafe attributes, such as `#[unsafe(naked)]` | Rejected only since 1.98: ``usage of the unsafe `#[naked]` attribute`` on 1.98, ``usage of the unsafe `naked` attribute`` since 1.99. 1.88 to 1.97 build it with no diagnostic. Search logs for `usage of the unsafe`, which matches both forms. |
| A `macro_rules!` defined in the same crate | The same block error at the macro body, with the call site marked `in this macro invocation` |
| A `macro_rules!` or a proc macro from a dependency crate | No diagnostic. The unsafe block runs, and an injected `#[unsafe(no_mangle)]` export builds. |

```rust,compile_fail
#![forbid(unsafe_code)]

// Rejected on every edition-2024 toolchain. Since 1.99 the message is
// "usage of the unsafe `no_mangle` attribute"; 1.98 prints `#[no_mangle]`
// in the backticks; before 1.98 it was "declaration of a `no_mangle` function".
#[unsafe(no_mangle)]
pub extern "C" fn exported() {}
```

A local `#[allow(unsafe_code)]` under the crate-level `forbid` is
`error[E0453]: allow(unsafe_code) incompatible with previous forbid`.

## Why a dependency macro escapes the lint

rustc suppresses lints on tokens that an external macro expanded. The boundary is the crate that
wrote the macro, not whether the macro is procedural. `cargo clippy -- -D unsafe_code` is silent
on the same code. No dependency filter closes the gap: `cargo tree -p <crate> | rg proc-macro`
lists nothing for a crate that exports a plain `macro_rules!`. Only the expansion shows every
case. Grep the expansion of each crate in a periodic audit. It needs nightly; keep it out of CI:

```bash
cargo +nightly rustc -p <crate> --lib --profile=check -- -Zunpretty=expanded \
  | rg -n 'unsafe \{|unsafe fn|unsafe impl|unsafe extern|#\[unsafe\('
```

Repeat with `--bin <name>` per binary: cargo passes `--` flags to one target. Hits from `std`
macros are expected; review the hits from dependency macros.

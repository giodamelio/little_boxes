# Lint catalog

What each lint in the canonical set buys, plus the optional blocks you add only when the
workspace needs them, and the lint rules for a binding layer.

- [Clippy lint groups](#clippy-lint-groups)
- [Defect class per lint](#defect-class-per-lint), with the
  [`ptr_arg` suppression matrix](#the-ptr_arg-suppression-matrix)
- [Optional block: pointer and FFI lints](#optional-block-pointer-and-ffi-lints)
- [Async lints: an explicit pin](#async-lints-an-explicit-pin), and `disallowed-types`
- [Binding-layer lints](#binding-layer-lints)
- [Lints on proc-macro output](#lints-on-proc-macro-output)
- [Lints deliberately not in the canonical set](#lints-deliberately-not-in-the-canonical-set)

## Clippy lint groups

Know the group before you set its level. A group level with `priority = -1`
sits below the individual lints, so a single lint can override its own group.

| Group | Contents | Recommended level |
|-------|----------|-------------------|
| `correctness` | Code that is almost certainly a bug | `deny` |
| `suspicious` | Code that is very likely wrong | `deny` |
| `style` | Idiomatic-Rust deviations | `warn` (pragmatic baseline) |
| `complexity` | Code that does something simple in a convoluted way | `warn` (pragmatic baseline) |
| `perf` | Code that is measurably slower than an equivalent | `warn` (pragmatic baseline) |
| `all` | The five groups above, combined | `deny` at `priority = -1`, in the strict template only. It covers the five groups above, so do not also set them: with `all` and `style` both at `priority = -1`, `style` lints come out `deny` (1.98.1). |
| `pedantic` | Opinionated, mostly correct, some noise | `warn` at `priority = -1` |
| `nursery` | New lints, still stabilizing | `warn` at `priority = -1` |
| `cargo` | Manifest and dependency-graph hygiene | `warn` at `priority = -1` |
| `restriction` | Deliberately restrictive; **never enable the group** | pick individual lints only |

The `perf` group is smaller than its name suggests. Verified on rustc 1.98.1:
`ptr_arg` and `unnecessary_lazy_evaluations` are in `style`, `assigning_clones`
is in `pedantic`, `redundant_clone`, `or_fun_call` and `needless_collect` are in
`nursery`, and `missing_asserts_for_indexing` is in `restriction`. Of the
performance lints in this catalog only `large_enum_variant` and
`result_large_err` are in `perf`, so a config that enables `clippy::perf` alone
gets those two and nothing else.

`restriction` as a group contradicts itself - it contains mutually exclusive
lints. Cherry-pick from it. The picks in the canonical set are
`indexing_slicing`, `integer_division`, `arithmetic_side_effects`,
`modulo_arithmetic`, `unwrap_used`, `panic`, `todo`,
`unimplemented`, `exit`, `str_to_string`, `allow_attributes`,
`allow_attributes_without_reason`, `undocumented_unsafe_blocks`,
`multiple_unsafe_ops_per_block`, `exhaustive_enums`, `exhaustive_structs`,
`unwrap_in_result`, `print_stdout`, `dbg_macro`.

## Defect class per lint

Read this table when you must justify a lint's level, or when you audit why a
defect reached production.

| Lint | Defect class it catches |
|------|-------------------------|
| `std::mem::forget` in `disallowed-methods` (or `mem_forget` = deny, not both) | Leaking `Drop` on a resource-bearing type: an open file, a native handle, a GPU surface, a lock guard. The resource is never released and nothing reports it. |
| `let_underscore_drop` = deny | `let _ = make_guard();` drops the new guard at the end of the statement, so the critical section that follows has no lock, and the code compiles cleanly. The lint also fires on `let _ = guard;` for a bound guard. That statement does not move or drop the guard: it lives to the end of its scope, so it does not release a lock early either. Only `drop(guard)` releases it. Verified on rustc 1.98.1. |
| `dropping_references` (rustc, warn by default) = deny | `drop(&guard)` instead of `drop(guard)`. `drop<T>(_: T)` is generic and `&G` is a valid `T`, so the call type-checks and does nothing; the guard lives to the end of its scope. Message: `calls to std::mem::drop with a reference instead of an owned value does nothing`. Verified on rustc 1.98.1: the compiler's own `help` suggests `let _ = &g;`, which is the same no-op with the warning gone. Neither form releases the guard. Only `drop(g)` does. Deny this lint together with `let_underscore_drop`. `let_underscore_drop` catches a guard that drops too early, and this lint catches a release call that releases nothing. |
| `undocumented_unsafe_blocks` | An `unsafe` block with no `// SAFETY:` comment. The invariant was never stated, so no reviewer can check it and no later author can preserve it. |
| `multiple_unsafe_ops_per_block` | One `// SAFETY:` comment covering several unsafe operations. The comment then justifies at most one of them. |
| `missing_safety_doc` | A `pub unsafe fn` with no `# Safety` rustdoc section. The caller has no stated contract to satisfy. |
| `unsafe_op_in_unsafe_fn` (rustc) | The whole body of an `unsafe fn` treated as one implicit unsafe block. This hides which line actually needs the audit. |
| `large_stack_arrays`, `large_stack_frames` | A big array or a fat state struct placed on the stack. It overflows on a small thread stack - worker threads and mobile main threads are far smaller than the default. |
| `large_futures` | An oversized future moved between tasks; each `spawn` copies it. |
| `large_enum_variant` | One huge variant inflating every value of the enum, including the small ones. It measures the size difference between the largest and the second-largest variant, strictly greater than `enum-variant-size-threshold` (default 200 bytes). It never looks at the total size. Measured on rustc 1.98.1 with `enum D { A(u8), B([u8; N]) }`: `N = 201` is silent, `N = 202` warns. So a 300-byte enum whose two largest variants are both near 300 bytes never fires. The fix is to box the outsized variant. It only wins when that variant is rare: boxing a hot variant buys size with an allocation on the common path. |
| `result_large_err` (warn by default) | A fat `Err` type carried by every `Result` in the call chain, on the success path as well. Measured on rustc 1.98.1: an `Err` variant of 128 bytes warns, 127 bytes is silent. Thresholds live in `clippy.toml`: `large-error-threshold` sets the byte count, `large-error-ignored` takes a type allow-list. The fix is to box the payload or to split the error type. |
| `assigning_clones` (pedantic) | `a = b.clone()` frees the buffer `a` already owns and allocates a new one. `a.clone_from(&b)` reuses it. The lint message is "assigning the result of `Clone::clone()` may be inefficient". Pedantic, so a plain `cargo clippy` never reports it. |
| `redundant_clone` (nursery) | A clone whose original is dead after the call, so the clone buys nothing. Nursery, so a plain `cargo clippy` never reports it. |
| `or_fun_call` (nursery) | `ok_or(build())`, `unwrap_or(build())`, `or_insert(build())`: the argument runs on every call, including the path that discards it. The fix is the `_else` form. Its counterpart `unnecessary_lazy_evaluations` (style, warn by default) catches the over-correction, a closure around a value that is cheaper to pass eagerly. Enable both, or a cleanup pass converts every eager call into a closure and trades one waste for another. |
| `needless_collect` (nursery) | A `collect()` whose result the same body only iterates, counts, or searches. Remove the collection and use the iterator. The lint does not look at return types: a function that returns `Vec<T>` to a caller that iterates once passes (clippy 0.1.98). Change such a function to return `impl Iterator<Item = T>` by hand. On edition 2024 that return type needs no lifetime bound, because a return-position `impl Trait` captures every in-scope lifetime by default; the same signature fails on edition 2021 with E0700. |
| `missing_asserts_for_indexing` (restriction) | Index sites where one `assert!` would let the compiler drop the bounds checks. It fires on `s[0] + s[1] + s[2]` with no preceding `assert!(s.len() > 2)`. It is the only automated way to find these sites. Restriction, so no default level and no config in this skill turns it on; run it as a one-off audit: `cargo clippy --locked --workspace -- -W clippy::missing_asserts_for_indexing`. |
| `ptr_arg` (style, not perf) | A `&Vec<T>` or `&mut Vec<T>` parameter where `&[T]` or `&mut [T]` works. It forces every caller to own a `Vec`. It is warn by default, so most projects see it, but a config that enables only `clippy::perf` misses it. The win is small and it is per call, not per iteration: measured on rustc 1.97.0 aarch64 at `-C opt-level=3`, for a body of `v.iter().sum()`, the `&Vec<u32>` version starts with two extra loads (the pointer and the length out of the `Vec` header) that the `&[u32]` version does not need, because a slice arrives in two registers. Both bodies came to 60 instructions. The suppression matrix below says when the owned reference is the right answer. |
| `exhaustive_enums`, `exhaustive_structs` | A public enum or struct without `#[non_exhaustive]`. Adding a field or a variant later is then a breaking change for every downstream match or literal. |
| `disallowed_methods` on `std::ptr::read` | A misaligned read from a byte buffer that came from I/O or FFI. `ptr::read` requires an aligned pointer, so a misaligned read is undefined behavior on every target. The optimizer can miscompile it on any target. It can fault on a strict-alignment target. aarch64 Linux and x86_64 permit a misaligned plain load (but not a misaligned atomic or exclusive access), so a passing test there does not prove the code correct. Use `read_unaligned`. The entry does not catch the `p.read()` method on `*const T` or `*mut T` (clippy 0.1.98); `cast_ptr_alignment` catches the cast that makes the pointer misaligned. |
| `disallowed_methods` on `std::env::set_var` | A mutation of the process environment after threads have started. It is not thread-safe. |
| `arithmetic_side_effects`, `integer_division`, `modulo_arithmetic` | Unchecked integer arithmetic: overflow, truncation, divide-by-zero. A release build wraps silently; a debug build panics. Both outcomes are wrong. `arithmetic_side_effects` fires on integer operators only, so it does not cover float math. |
| `indexing_slicing` | `buf[i]` on data whose length came from outside the program. Use `.get()` and handle the `None`. |
| `unwrap_used`, `expect_used`, `unwrap_in_result` | A panic path in non-test code, usually written where an error should have propagated. `expect_used` belongs only in a crate where no panic may escape; see the last table. |
| `panic`, `todo`, `unimplemented` | A stub that compiles and ships. `todo!()` in a merged branch is a runtime crash waiting for the right input. |
| `allow_attributes`, `allow_attributes_without_reason` | A bare `#[allow(...)]` smuggling a violation past review with no justification. Also pushes authors to `expect`, which self-reports when it goes stale. |
| `missing_docs`, `missing_panics_doc`, `missing_errors_doc` | A public API whose failure and panic conditions are undocumented, so every caller guesses. |
| `dbg_macro`, `print_stdout` | Debug output left in a shipped build. On a library or a CLI with structured output, `println!` corrupts the protocol. |
| `exit` | `process::exit` inside library code. It skips every destructor and every buffer flush. |
| `redundant_closure_for_method_calls` | A closure that only forwards to a method, where the method path itself works. |
| `uninlined_format_args` | `format!("{}", x)` instead of `format!("{x}")`. Cheap to fix and it removes review noise. |
| `implicit_clone`, `str_to_string`, `cloned_instead_of_copied` | Allocation nobody asked for, on a hot path nobody measured. |
| `needless_pass_by_value`, `trivially_copy_pass_by_ref` | An ownership signature that fights the caller in both directions. |
| `disallowed_types` | A type that the project replaced, for example std `HashMap` where the project uses one chosen hasher. |
| `unused_must_use` (rustc) | A dropped `Result` or a dropped guard-like value. |
| `unreachable_pub` (rustc) | An item marked `pub` that no external path can reach. It inflates the apparent API surface. |
| `non_ascii_idents` (rustc) | Homoglyph identifiers. Two distinct items look identical in review. |
| `refining_impl_trait` (rustc, warn by default) | An impl of a trait method that returns `impl Trait` writes the concrete type instead of repeating the opaque one. Every caller who holds the impl type then sees more than the trait promised, so the refinement is public API that nobody chose. Message: `impl trait in impl method signature does not match trait method signature`. The sub-lint is `refining_impl_trait_reachable` on a reachable impl and `refining_impl_trait_internal` on a private one; both sit under the `refining_impl_trait` group. Repeat the opaque form in the impl, with the same `use<..>` capture list. Suppress it only when the concrete type is meant to be public API: put `#[expect(refining_impl_trait, reason = "...")]` on the impl method, not `#[allow]` (verified on 1.98.1). |
| `rustdoc::broken_intra_doc_links` = deny | Documentation that silently rots after a rename. |

### The `ptr_arg` suppression matrix

`ptr_arg` fires on the parameter type, not on the call site. Two facts decide every probed case:
`str` has no `capacity`, and `[T]` cannot change its length. Everything else is reachable through
the slice, so the owned reference buys nothing. Silence can also be a false negative: on clippy
0.1.98 the lint is silent on a `&String` whose body calls `s.as_bytes()`, although `str` has
`as_bytes`.

Verified with clippy 0.1.98 (Rust 1.98.1) on edition 2024. Each line is one function in one crate:

```rust
pub fn a(s: &String) -> usize { s.len() }         // fires -> &str
pub fn b(s: &String) -> usize { s.capacity() }    // silent
pub fn c(s: &String) -> String { s.clone() }      // fires -> &str plus s.to_owned()
pub fn d(v: &mut Vec<u8>) { v.push(1); }          // silent
pub fn e(v: &mut Vec<u8>) { v[0] = 1; }           // fires -> &mut [u8]
pub fn f(v: &Vec<u8>) -> Vec<u8> { v.clone() }    // fires -> &[u8] plus v.to_owned()
```

The full result of a 34-function probe on clippy 0.1.98, all four parameter types. A dash marks a
combination the probe did not cover:

| Method the body calls | `&String` | `&mut String` | `&Vec<T>` | `&mut Vec<T>` |
| --- | --- | --- | --- | --- |
| `len`, `is_empty`, `chars` or `iter`, an index or slice read | fires | - | fires | - |
| `clone` | fires, and rewrites to `to_owned()` | - | fires, and rewrites to `to_owned()` | fires |
| `capacity`, `reserve` | silent | silent | silent | silent |
| `push`, `pop`, `truncate`, `insert`, `remove`, `clear` | - | silent | - | silent |
| `make_ascii_uppercase`, `sort`, `v[0] = x` write | - | fires, to `&mut str` | - | fires, to `&mut [T]` |

Three non-obvious consequences:

- Cloning does not justify the owned reference. Clippy rewrites `s.clone()` on a `&str` parameter
  to `s.to_owned()` and still asks for the slice.
- Writing through a `&mut Vec<T>`, by index or by `sort`, does not justify it either. Only a
  length-changing call does.
- `reserve` counts as capacity work, not as length change, so it does suppress the lint.

Keep the owned reference when a suppressed row applies, and write the reason in a comment. Add no
`#[allow(clippy::ptr_arg)]`: if the lint is silent, there is nothing to allow, and if it fires the
signature is wrong.

## Optional block: pointer and FFI lints

Add these only in a workspace that contains hand-written pointer work. In a
workspace where every crate is `#![forbid(unsafe_code)]` they can never fire.

```toml
[workspace.lints.clippy]
mut_from_ref              = "deny"   # returning &mut from &, an aliasing violation
transmute_ptr_to_ptr      = "deny"
useless_transmute         = "deny"
crosspointer_transmute    = "deny"
cast_ptr_alignment        = "deny"   # a cast that raises the alignment requirement
transmute_undefined_repr  = "warn"
as_ptr_cast_mut           = "warn"
ptr_as_ptr                = "warn"   # prefer .cast() over `as *const T`
rc_mutex                  = "deny"   # Rc<Mutex<T>> is single-threaded; you wanted Arc or RefCell

[workspace.lints.rust]
improper_ctypes            = "warn"  # a non-FFI-safe type in an extern block
improper_ctypes_definitions = "warn" # the default; never allow it crate-wide
```

`cast_ptr_alignment` and the transmute family are the static half of the
alignment story. Miri is the runtime half. Miri cannot run foreign code, so cover FFI paths
with ASan on the host or HWASan on a device. Run Miri with `-Zmiri-symbolic-alignment-check` for a read through a cast byte
pointer (the `rust-unsafe` and `rust-sanitizers-miri` skills).

## Async lints: an explicit pin

`await_holding_lock`, `await_holding_refcell_ref`, and `await_holding_invalid_type` are in
`clippy::suspicious` and warn by default (clippy 0.1.99; check with
`cargo clippy -- -W help | grep await-holding`). The strict template (`all = "deny"`) and the
pragmatic baseline (`suspicious = "deny"`) therefore already deny them. The block below only
pins two of them by name, so they stay on if a group level is relaxed later. Add it when the
workspace runs an async runtime. Merge the keys into the existing `[workspace.lints.clippy]`
table: a second table header, or a key the table already has, is a TOML parse error. The strict
template in [workspace-lints.md](workspace-lints.md) already sets `large_futures`.

```toml
await_holding_lock         = "deny"  # a blocking guard held across .await deadlocks the executor
await_holding_refcell_ref  = "deny"  # a RefCell borrow held across .await panics on re-entry
```

`await_holding_invalid_type` checks nothing until `clippy.toml` lists the types. Add the key
only with a type to ban, for example a guard from your own crate:

```toml
# clippy.toml
await-holding-invalid-types = [
  { path = "your_crate::TxnGuard", reason = "commit or drop the transaction before .await" },
]
```

Measured on clippy 0.1.99: with that key, the listed type held across `.await` gives `holding a
disallowed type across an await point`; without it, nothing fires.

Do not ban `std::sync::Mutex` or `RwLock` with `disallowed-types`.
`clippy.toml` cannot scope a ban to async modules, and a std lock whose guard
does not cross `.await` is the right choice in async code.
`await_holding_lock` already catches the guard that does cross it. The
`rust-async-internals` skill owns the lock choice: a guard held across `.await` needs
`tokio::sync::Mutex`.

>>>

`disallowed-types` enforces one hasher across the project.

```toml
disallowed-types = [
  { path = "std::collections::HashMap", reason = "use rustc_hash::FxHashMap" },
]
```

Clippy matches the written path, not the resolved type. Measured on rustc
1.97.0 with rustc-hash 2.1.3, that one entry gives three warnings for a single
std `HashMap` use - the `use` import, the return-type annotation, and the
`HashMap::new()` call - and zero warnings for an `FxHashMap` in the same file,
although `FxHashMap<K, V>` is an alias for `std::collections::HashMap<K, V,
FxBuildHasher>`. The alias passes because its path differs. That is what makes
the rule usable: it names the hasher you want without banning the map. Choose
the hasher first; `FxHashMap` is not HashDoS-resistant, so it needs keys that
no attacker controls. Keep std `RandomState` for keys from
outside the process (the `rust-hot-path` and `rust-security` skills).

## Binding-layer lints

A raw JNI layer can trip two lints, because the foreign ABI fixes the entry-point signatures.
Both fire only on the signature styles in the table (verified on 1.98.1). An entry point that
takes the `jni` crate's wrapper types trips neither.

| Lint | When the JNI layer trips it |
|------|-----------------------------|
| `missing_safety_doc` (style, warn) | The entry points are `pub unsafe extern "system"` functions with no `# Safety` section. The JVM is the only caller. |
| `not_unsafe_ptr_arg_deref` (correctness, deny) | A safe `pub extern "system"` function dereferences a raw `JNIEnv` or object pointer argument. |

Fix an export that takes a raw pointer by making it `unsafe`, with a `# Safety` section that
states what the caller must guarantee. That signature satisfies both lints. Measured on clippy
0.1.99 with `#[unsafe(no_mangle)]` exports that dereference a `*const u8`: a safe `pub extern
"C" fn` is a `not_unsafe_ptr_arg_deref` deny error, a `pub unsafe extern "C" fn` without
`# Safety` warns `missing_safety_doc`, the same with `# Safety` is clean, and a non-`pub` safe
`extern "C" fn` gets no lint at all. The last form only hides the defect: the symbol is still
exported, and the foreign caller still passes a raw pointer to a function that claims to be safe.
Choose visibility separately, for the Rust callers of the crate. Never suppress
`not_unsafe_ptr_arg_deref` (SKILL.md, Crate-level attributes), and do not relax either lint for
the binding crate or the workspace.

```rust
/// Reads the first byte of a buffer the foreign caller owns.
///
/// # Safety
///
/// `p` must be non-null, aligned, and valid for a one-byte read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn first_byte(p: *const u8) -> u8 {
    // SAFETY: the caller upholds the contract in `# Safety`.
    unsafe { *p }
}
```

Keep `improper_ctypes_definitions` at its default level and never allow it crate-wide. The
`rust-jni` skill owns the entry-point signatures (`JNI_OnLoad` takes `*mut jni::sys::JavaVM`), and
the `uniffi-boundary` skill owns generated UniFFI scaffolding (one `uniffi::setup_scaffolding!()`
in `lib.rs`).

## Lints on proc-macro output

rustc and clippy suppress almost every lint on tokens that another crate's proc macro emitted,
`unsafe_op_in_unsafe_fn` included. A `macro_rules!` defined in the same crate is not exempt.

1. Lints that read an item signature still fire. `clippy::ptr_arg` and
   `clippy::exhaustive_structs` report `this warning originates in the macro`. Handle them with
   a narrow crate-level `#![expect(..., reason = "...")]`, not a workspace change.
2. `#![forbid(unsafe_code)]` is a soundness gate, and the silence removes the guarantee. A crate
   that carries it builds and runs the `unsafe` that a dependency's proc macro injected, and
   `cargo clippy -- -D unsafe_code -D clippy::undocumented_unsafe_blocks` reports nothing.
   Audit the proc-macro dependencies of a binding crate: grep the nightly
   `-Zunpretty=expanded` output for `unsafe` (the `rust-unsafe` skill).

Verified on rustc 1.98.1: a proc macro emits an unused variable, an unused `mut`, and a
non-snake-case function into a crate with `#![warn(unused_variables, unused_mut,
non_snake_case)]`, and the build reports nothing. The same source written by hand reports four
warnings. `#![deny(unsafe_op_in_unsafe_fn)]` is silent on a macro-emitted `pub unsafe fn f(p:
*const u8) -> u8 { *p }`, which fails with E0133 when you write it yourself.

## Lints deliberately not in the canonical set

| Lint | Why not |
|------|---------|
| `clippy::string_to_string` | Deprecated in clippy 1.91. `implicit_clone` covers it. |
| `expect_used` in the workspace table | A `.expect("<invariant>")` is allowed in non-test code (the `rust-discipline` skill). Add the lint per crate where no panic may escape, such as an FFI adapter, beside `clippy::unwrap_used` and `clippy::panic` (the `rust-panic-safety` skill). |
| `clippy::restriction` (the whole group) | Contains mutually exclusive lints. Cherry-pick instead. |
| An explicit `await_holding_*` pin in a synchronous workspace | The group level already sets them, and they cannot fire without `.await`. |
| `unsafe_code = "forbid"` in `[workspace.lints.rust]` | Set it per crate; see SKILL.md, Crate-level attributes. |

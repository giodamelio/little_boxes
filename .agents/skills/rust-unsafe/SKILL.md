---
description: Use when adding, removing, or reviewing unsafe Rust, such as unsafe blocks, unsafe impl Send or Sync, raw pointers, transmute, mem::zeroed and MaybeUninit, unions, hand-written extern "C" and no_mangle exports, syscall or ioctl wrappers, zero-copy buffers, mmap, and forbid(unsafe_code). Also for soundness or undefined-behavior questions. Triggers on "SAFETY comment", "Strict Provenance", "repr(packed)", "E0793", "E0133", "improper_ctypes", "OwnedFd".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-unsafe
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 482db65f43a077e2ac0155d0a25b445ac34d7532
name: rust-unsafe
---
# Rust unsafe

## Find the unsafe surface

Inventory unsafe code before an audit or change:

```bash
# Crates that promise no unsafe in their own source.
rg -l '#!\[forbid\(unsafe_code\)\]' --type rust

# Where unsafe code lives.
rg -n 'unsafe\s*\{|unsafe fn|unsafe impl|unsafe extern' --type rust

# Symbols that leave the crate unmangled, and custom link sections.
rg -n '#\[(unsafe\()?(no_mangle|export_name|link_section)' --type rust
```

## Governance with `#![forbid(unsafe_code)]`

Enforce a safe crate with `#![forbid(unsafe_code)]`. Add it
to every crate root that has no hand-written `unsafe`, including a crate that reaches C or the OS
only through a safe wrapper crate. A local `#[allow(unsafe_code)]` cannot weaken it (E0453). A
crate with an `#[unsafe(no_mangle)]` export cannot carry it. A dependency macro that expands to
unsafe escapes it, so a `forbid` hit does not prove the crate compiles to no unsafe. Remove the
attribute only in the commit that adds the unsafe code, and state which operation forced it. Keep
the set of unsafe crates small. Read [references/forbid-unsafe-code.md](references/forbid-unsafe-code.md)
for the result per source of unsafe and the nightly macro-expansion grep.

## Lint floor for unsafe crates

Put these lints at the root of every crate that contains `unsafe`:

```rust
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block,
    clippy::missing_safety_doc,
)]
```

They require an `unsafe { ... }` block per unsafe operation inside an `unsafe fn`, a
`// SAFETY:` comment above every block, one unsafe operation per block, and a `# Safety` section
on every `pub unsafe fn`. By default the two restriction lints are off. Set the lints in
`[workspace.lints]`, and restate them at the root of each crate that contains unsafe.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `E0133: ... is unsafe and requires unsafe block` (or the `unsafe_op_in_unsafe_fn` warning) | An unsafe or foreign `fn` call, a `static mut` or `extern` static access, or a `#[target_feature]` fn called without that feature | One `unsafe { }` per operation, with a SAFETY comment; declare a foreign fn `safe fn` when it has no precondition; or give the caller the same `#[target_feature]` |

When you add the first unsafe to a safe crate, add the floor and run
`cargo clippy --locked -p <crate> --all-targets -- -D warnings` before you write the unsafe body.
Performance is a reason for unsafe only after a measurement. Read
[references/audit-checklist.md](references/audit-checklist.md) when you decide whether a new
unsafe site is legitimate.

For Rust 1.100 custom allocators, `conjure_zst`, or `UnsafeCell` access, read
[the API rules](references/allocator-and-zst.md).

## Verify an unsafe change

Match checks to risk.

1. Every change: `cargo clippy --locked -p <crate> --all-targets -- -D warnings`. It proves that
   the lint floor holds. It does not prove that a SAFETY comment is true.
2. Every change: `cargo test --locked -p <crate>` in the debug profile. Debug assertions
   check many std unsafe preconditions and a misaligned or null `*p` on the executed path, and
   abort with `unsafe precondition(s) violated: ...`. Treat that as undefined behavior.
   `--release` removes these checks.
3. Code that Miri can execute, when nightly `miri` is installed:
   `cargo +nightly miri test --locked -p <crate>` (Stacked Borrows), then
   `MIRIFLAGS="-Zmiri-tree-borrows"` as a second opinion, never as an excuse for a default-model
   failure.
4. FFI code that Miri cannot execute: `#[cfg_attr(miri, ignore)]`, and ASan on the host, or
   HWASan or MTE on a device. A `#[cfg(miri)]` stub must dereference every pointer that the
   foreign side stores.

Read `references/miri-and-aliasing.md` ("Miri invocations") for the flags that each code shape
needs and for what a clean run does not prove.

The change is complete when clippy passes, a debug test executes each unsafe path, every block
and `unsafe impl` has a SAFETY comment that names its invariant, and the report names each unsafe
path that neither Miri nor a sanitizer executed. Read
[references/audit-checklist.md](references/audit-checklist.md) when you review unsafe code that
someone else wrote, or before a merge that adds unsafe.

## Triage: faults that pass a safety review

A correct unsafe block cannot fix the wrong data shape.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `E0793: reference to field of packed struct is unaligned` | `&` or `&mut`, a `&self` or `&mut self` method (`clone()`), a `match` guard, or a `ref` binding on a `#[repr(packed)]` field | Copy the field out, or `(&raw const f).read_unaligned()` |
| Default Miri or a debug `*p` fails only on some buffer addresses | `bytes.as_ptr() as *const u32`, then a read | "Pointer reads from untrusted byte buffers" below |
| `error: casting &T to &mut T is undefined behavior` | Mutation through a shared reference | `UnsafeCell`; there is no other sound way |
| A `*const` silently became a `*mut` | `as` on a pointer changes mutability too | `ptr.cast::<T>()`, and `cast_mut` when you mean it |
| `warning: uses type dyn Trait, which is not FFI-safe` | A fat pointer in a C signature | Pointer and length, or an opaque handle |
| Two unrelated handles accepted at one call | Every handle is `*mut c_void` | One zero-sized `#[repr(C)]` opaque type per handle |
| Memory grows per callback registration | The boxed closure context is never reclaimed | An unregister path that calls `Box::from_raw` |
| Double close, or a read from a reused descriptor | `RawFd` states no owner | `OwnedFd` to own, `BorrowedFd<'_>` to lend |
| A field is corrupt on one target only | Hand-rolled bitfield masks | A bitfield crate and a round-trip test |
| Sound in debug, unsound in release | `debug_assert!` guards an unsafe block | `assert!`, which survives the shipping profile |
| A handle outlives the data it points at | A raw pointer carries no lifetime | `PhantomData<&'a T>` to borrow, `PhantomData<T>` to own |

Keep both FFI-safety lints on: `improper_ctypes` checks imports, and
`improper_ctypes_definitions` checks exports. Never allow either crate-wide; put
`#[expect(..., reason = "...")]` on the one item that the lint cannot judge.
Since 1.98, `c_void_returns` warns on `-> c_void`; a C `void` function has no return type.
Read [references/ffi-layout-rules.md](references/ffi-layout-rules.md) when you apply a fix from
this table.

## Drop hazards

Soundness must not depend on a `Drop` running: `mem::forget`, `Box::leak`, and an `Rc` cycle skip
it in safe code. Read
`references/unsafe-patterns.md` when a guard, a registration, or a `Drop::drop` can fail.

## One unsafe block breaks local reasoning

One unsafe block anywhere in the call graph, a dependency included, can break an invariant
that safe code relies on: a `str::from_utf8_unchecked` on unvalidated bytes surfaces in your
first decode loop, and neither the compiler nor Miri reports the construction. When you audit:

1. `rg 'from_utf8_unchecked|from_raw_parts' --type rust -n`. Every hit
   needs a SAFETY comment that traces back to where the invariant is established.
2. Run `cargo deny --config deny.toml --locked check advisories` to find a dependency with a
   known soundness advisory. The `rust-security` skill, when it is installed, owns the policy;
   set `unsound = "all"` so that a transitive unsound crate also fails.
3. When a dependency's unsafe transits through your API, restate its invariant in your
   `# Safety` section.

Read [references/unsafe-patterns.md](references/unsafe-patterns.md) when you write the
reproducing test; only a decode loop fails on invalid UTF-8.

## Manual `Send` or `Sync` impls and reference fabrication

A manual `unsafe impl Send` remains unchecked after a field gains an `Rc`.
Bound its parameters (`unsafe impl<T: Sync> Sync`); audit each field and `&self` method. Never hand out a `&T` built from `RefCell::as_ptr` or a `&String` from
`String::from_raw_parts` over a `&str`; Miri can pass both. Read
`references/miri-and-aliasing.md` before you write or review either.

## Pointer reads from untrusted byte buffers

`std::ptr::read(buf.as_ptr() as *const T)` requires an aligned pointer. Bytes from I/O, a
mapping, or an FFI caller carry arbitrary alignment, and a misaligned `ptr::read` is undefined
behavior on every target, although native x86-64 and AArch64 tests pass.

1. Read a `&[u8]` from I/O, FFI, or a mapping with `ptr::read_unaligned`, even when the field is
   aligned today.
2. Prefer `zerocopy::FromBytes`, `u32::from_le_bytes`, or `bytes::Buf` over pointer arithmetic
   on byte buffers. They remove the unsafe block and make endianness explicit.
3. Test every new unsafe byte parser under Miri with `-Zmiri-symbolic-alignment-check`; default
   Miri fails only on the seeds where the buffer lands misaligned.
4. Run a parser that uses native endianness or pointer casts under the big-endian
   `cargo +nightly miri test --target s390x-unknown-linux-gnu`.

Grep audit:

```bash
rg -n 'as_ptr\(\)\s*(as\s*\*const|\.cast::<)' --type rust | rg -v 'read_unaligned|::<u8>'
rg 'transmute::<\s*&\[u8\]' --type rust -n
```

The grep misses a cast in a later statement; `clippy::cast_ptr_alignment` (pedantic) catches it.
Read
[references/unsafe-patterns.md](references/unsafe-patterns.md) when you write a byte parser.

## Where the other patterns live

- `mem::zeroed`, `assume_init`, a reference from a raw pointer, or an integer-to-pointer
  conversion: `references/validity-and-provenance.md`.
- A slice over a foreign buffer or a mapping, or an `#[unsafe(no_mangle)]` export:
  `references/ffi-layout-rules.md`.
- Whether a block needs `unsafe`; an `unsafe impl`, `unsafe trait`, unsafe attribute, or
  `# Safety` section; pointer arithmetic; a syscall, `ioctl`, or descriptor wrapper:
  `references/unsafe-patterns.md`.

## Panics and unwinding at the FFI boundary

The `rust-panic-safety` skill, when it is installed, owns the panic policy: wrap each
`extern "C"` entry body in `catch_unwind` and ship `panic = "unwind"`. Two soundness facts stay
here:

- Below `rust-version` 1.81, an unwind out of `extern "C"` is undefined behavior, not an abort.
- A foreign exception that unwinds into Rust through an import declared `extern "C"` is
  undefined behavior. Declare such an import `extern "C-unwind"`.

Read [references/unsafe-patterns.md](references/unsafe-patterns.md) when you write a hand-rolled
`extern "C"` entry point, or a `from_raw` call that takes ownership of a foreign handle.
`JNI_OnLoad` returns `JNI_ERR` on a caught panic.

## Transmute

Prefer the named conversion, such as `f32::from_bits`, `u32::from_ne_bytes`, `Box::into_raw`, or
`zerocopy::FromBytes`. When none exists, write both types in the turbofish and state the layout
and lifetime argument in the SAFETY comment.
Read [references/unsafe-patterns.md](references/unsafe-patterns.md) when you choose a conversion
or extend the lifetime bound of a `dyn` raw pointer.

## Related skills

When installed: `rust-jni` and `uniffi-boundary` (boundaries), `rust-variance`
(`PhantomData`), `rust-pin-projection` (`map_unchecked_mut`), `memory-model` (atomics), and
`rust-debugging` (a crash).

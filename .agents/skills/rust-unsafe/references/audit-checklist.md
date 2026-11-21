# Unsafe audit checklist

Use this list when you review unsafe code that someone else wrote, or before a merge that adds
unsafe. [SKILL.md](../SKILL.md) explains the rules. Answer every item that applies to the diff.
Skip the items that do not apply.

## Is the unsafe legitimate?

| Case | Verdict |
| --- | --- |
| An FFI export or import: `extern "C"`, `extern "system"`, handle construction | Legitimate |
| An OS-level call with no safe wrapper: `ioctl`, `setsockopt`, signal handling | Legitimate |
| A zero-copy handoff of a large buffer, after a copy was measured and rejected | Legitimate |
| SIMD intrinsics on a hot path, after a benchmark justified them | Legitimate |
| Protocol and format parsing | Use `zerocopy` or `bytes` under `#![forbid(unsafe_code)]` |
| Domain logic, configuration, state | Use `#![forbid(unsafe_code)]` |
| Anything a safe crate already wraps | Use the crate |

Performance is a reason only after a measurement. The `rust-performance` skill, when it is
installed, shows how to produce one with `samply` or `cargo flamegraph`. If a new site fits no row, add the case and say why it is
acceptable.

## Every unsafe block

- [ ] Does the block do one of the five unsafe operations? If not, delete the block.
- [ ] Is there a `// SAFETY:` comment, and does it name the invariant that the block relies on?
- [ ] Does each block hold one unsafe operation, so each invariant has its own comment?
- [ ] Is the block as small as it can be?
- [ ] Does a check that guards the block use `assert!`, not `debug_assert!`?
- [ ] For a raw pointer: is it non-null, aligned, initialized, and valid for the whole access?
- [ ] For pointer arithmetic: does the result stay in the same allocation, and is a cursor
      assigned back after `add`?

## Values, layout, and provenance

- [ ] For `mem::zeroed()`: does the documented contract of the exact type prove that all-zero
      bytes is a valid value?
- [ ] Does every `assume_init` follow a proof that all bytes form a valid value of the exact
      type?
- [ ] Does integer-address manipulation keep provenance with `addr`, `with_addr`, or
      `map_addr`, or document why exposed provenance is required?
- [ ] For a union read: does the storage meet the read type's initialization, validity,
      pointer provenance, lifetime, and aliasing requirements?
- [ ] Does any reference point into a `#[repr(packed)]` struct, or into unaligned bytes?
- [ ] Does a read from a byte buffer use `read_unaligned`, `zerocopy`, or `from_le_bytes`?

## FFI boundary

- [ ] Does each foreign-callable entry point follow the crate's panic policy: `catch_unwind`
      when the caller needs a status, a deliberate abort otherwise? If `rust-version` is below
      1.81, is every entry point wrapped? There the unwind is undefined behavior, not an abort.
- [ ] Does the release profile set `panic = "abort"`, which makes every `catch_unwind` inert?
- [ ] Is an import that a foreign exception can unwind through declared `extern "C-unwind"`?
- [ ] For a `from_raw` handle: is the raw value valid and consumed exactly once?
- [ ] For a slice over a foreign buffer: is exclusive access guaranteed for the whole call?
- [ ] For a mapping: does the mapping outlive every slice built over it?
- [ ] Does any C signature carry a fat pointer: `dyn Trait`, a slice, or `&[T]`?
- [ ] Does every boxed callback context have an unregister path that reclaims it?
- [ ] Does any unmangled symbol collide with one in another library loaded at the same time?

## Ownership, threads, and destructors

- [ ] For `unsafe impl Send` or `Sync`: does every field, and every `&self` method including
      derives, keep the promise?
- [ ] Does soundness depend on a destructor that a caller can skip with `mem::forget`?
- [ ] Does any `Drop::drop` contain `.unwrap()`, `.expect()`, or another panicking call?
- [ ] Does any API hand out a reference built from `RefCell::as_ptr`?

## Evidence

- [ ] Did the debug-profile tests run the unsafe path?
- [ ] Did the tests run under default Miri, with `-Zmiri-symbolic-alignment-check` for byte
      parsers, and under Tree Borrows only as a second pass?
- [ ] For FFI code that Miri cannot run: did an ASan, HWASan, or MTE run cover the path, plus
      `cargo-careful` or a Miri stub that dereferences the stored pointer?

# FFI layout and pointer-shape rules

Rules about the *shape* of data that crosses a boundary: alignment, pointer width, field
layout, and handle ownership. [SKILL.md](../SKILL.md) covers the safety contract of an unsafe
block. This file covers the cases where the contract is met and the layout is still wrong.

The repository compile check type-checks these examples on its pinned toolchain, edition 2024.

Contents:

- Packed structs (misaligned byte reads are in [unsafe-patterns.md](unsafe-patterns.md))
- A slice over a caller-owned buffer or a mapping
- `&T` to `&mut T`; `pointer::cast` over `as`
- Fat pointers, opaque handles, and closures across a C boundary
- `OwnedFd` and `BorrowedFd`; bitfields; `assert!` over `debug_assert!`
- Rust 1.100 layout diagnostics
- `PhantomData` markers for raw-pointer structs
- `union` for C interop only
- Symbol collision in `cdylib` crates
- Zero-copy buffer handoff

## Rust 1.100 layout diagnostics

Rust 1.100 reports repeated `#[repr(..)]` attributes. Combine the intended
representations in one attribute and remove duplicates. An aligned field inside
a packed type now triggers the deny-by-default `aligned_fields_in_packed` lint.
Keep the error policy; lowering it does not make references to packed fields aligned.
The x86 Rust ABI also changes register use. Keep a stable foreign boundary on an
explicit ABI such as `extern "C"`; do not exchange Rust-ABI objects across compiler versions.

## Never reference a field of a `#[repr(packed)]` struct

A packed struct has no padding, so a field can sit at a misaligned address. A reference must
always be aligned. Since Rust 1.69 the compiler rejects the reference outright:

```text
error[E0793]: reference to field of packed struct is unaligned
  = note: creating a misaligned reference is undefined behavior
          (even if that reference is never dereferenced)
```

A borrow (`&` or `&mut`), a method that takes `&self` or `&mut self` (autoref, such as `clone()`
or `to_string()`), a `match` arm with a guard, and a `ref` or `ref mut` binding all create a
reference. They fail with E0793. A plain `match` or `if let` on a `Copy` field, with no guard and
no `ref` binding, reads a copy and compiles. A method that takes `self` by value, such as
`count_ones()`, also compiles. Copy the field out, or go through a raw pointer:

```rust
#[repr(C, packed)]
pub struct Packet {
    pub header: u8,
    pub value: u32,   // offset 1, not 4
}

// Copying is safe: no reference is created.
pub fn value_copy(p: &Packet) -> u32 {
    p.value
}

// Reading through a raw pointer is safe when the read tolerates misalignment.
pub fn value_read(p: &Packet) -> u32 {
    // SAFETY: `&raw const` never creates a reference, and `read_unaligned` does
    // not require alignment. The field is initialized because `p` is a reference
    // to a fully initialized `Packet`.
    unsafe { (&raw const p.value).read_unaligned() }
}

pub fn set_value(p: &mut Packet, value: u32) {
    // SAFETY: same as above; `write_unaligned` does not require alignment.
    unsafe { (&raw mut p.value).write_unaligned(value) }
}
```

Use `&raw const` and `&raw mut`. They are the native syntax since Rust 1.82 and they replace
`ptr::addr_of!` and `ptr::addr_of_mut!`. The macros still work, but the operator is the form to
write in new code.

The rustc lint `unaligned_references` was removed when E0793 became a hard error (1.69). Naming
it gives a `renamed_and_removed_lints` warning, and a `-D warnings` gate turns that warning into a
failure.

Prefer a layout that avoids the problem. A byte-array field needs no packing and no unaligned
access:

```rust
#[repr(C)]
pub struct PacketBytes {
    pub header: u8,
    pub value: [u8; 4],
}

impl PacketBytes {
    // No unsafe at all. The wire order is explicit, which also fixes endianness.
    pub fn value(&self) -> u32 {
        u32::from_be_bytes(self.value)
    }
}
```

## A slice over a caller-owned buffer or a mapping

State all three guarantees in the SAFETY comment, and state them again at the foreign call site:

```rust
// SAFETY: `ptr` points at the start of a caller-allocated RGBA8 buffer of
// `width * height * 4` bytes. The caller guarantees:
//   1. Non-null, and aligned for `u32`.
//   2. Exclusively writable: no concurrent read or write from the caller.
//   3. Valid for the entire duration of this call.
let width = usize::try_from(width).map_err(|_| Error::DimensionsOverflow)?;
let height = usize::try_from(height).map_err(|_| Error::DimensionsOverflow)?;
let pixel_count = width
    .checked_mul(height)
    .filter(|count| *count <= isize::MAX as usize / std::mem::size_of::<u32>())
    .ok_or(Error::DimensionsOverflow)?;
let pixels = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<u32>(), pixel_count) };
```

The same rule covers a slice built over a memory-mapped region. The mapping must outlive the
slice, and the region must not be mutated while the slice is live:

```rust
/// # Safety
/// `base` must be the start of a valid mapping of at least `len` bytes, and
/// `len` must not exceed `isize::MAX`. The mapping must stay alive for `'map`,
/// and must not be written while the returned slice is live.
unsafe fn mmap_as_slice<'map>(base: *const u8, len: usize) -> &'map [u8] {
    assert!(len <= isize::MAX as usize);
    // SAFETY: the caller guarantees the mapping is valid, read-only, and lives
    // for at least `'map`.
    unsafe { std::slice::from_raw_parts(base, len) }
}
```

A fabricated lifetime like `'map` above is a promise the compiler cannot check. Keep the
function private, and make the owning type hold the mapping so the borrow checker enforces the
relationship for every caller.

## Never cast `&T` to `&mut T`

rustc rejects it through the deny-by-default lint `invalid_reference_casting`. Never allow the
lint: the cast is undefined behavior even when the reference is unused.

```text
error: casting `&T` to `&mut T` is undefined behavior, even if the reference is unused,
       consider instead using an `UnsafeCell`
  = note: `#[deny(invalid_reference_casting)]` on by default
```

The clippy lint `cast_ref_to_mut` was renamed to the rustc lint `invalid_reference_casting`. If
you need mutation through a shared reference, the type must contain an `UnsafeCell`. There is no
other sound way to get one.

`UnsafeCell` makes the cast legal. It does not make the design sound. A parameter-extraction
layer that yields `&mut T` out of a shared `&Store` runs its extractor once per parameter, with
no knowledge of the other parameters, so two parameters of the same type produce two live `&mut`
to one allocation. Nothing in the type system rejects that call. Carry an access set per call,
record the type of every `&mut` parameter, and fail the registration on a conflict. That runtime
check is why every entity-component framework has one.

## Prefer `pointer::cast` over `as`

```rust
pub fn to_u32(p: *const u8) -> *const u32 {
    p.cast::<u32>()
}
```

`as` on a pointer silently changes mutability as well as the pointee type. `cast` changes only
the pointee type, so a `*const` cannot become a `*mut` by accident. Use `cast_mut` and
`cast_const` when you do intend the change, so the intent is written down. Enable
`clippy::ptr_as_ptr`.

## Never put a fat pointer in a C signature

A trait object pointer and a slice pointer are two words wide. A C pointer is one word:

| Type | Size on a 64-bit target |
| --- | --- |
| `*const u8` | 8 |
| `*const dyn Trait` | 16 |
| `*const [u8]` | 16 |
| `&[u8]` | 16 |

C cannot build the second word, and it cannot read it. The compiler warns:

```text
warning: `extern` block uses type `dyn T`, which is not FFI-safe
  = note: trait objects have no C equivalent
  = note: `#[warn(improper_ctypes)]` on by default
```

`improper_ctypes` covers declarations you import. `improper_ctypes_definitions` covers
`extern "C"` functions you export. Keep both on, and never allow either crate-wide. Since 1.98,
`c_void_returns` also warns on a `-> c_void` return type:

```text
warning: declarations returning `c_void` are not compatible with C functions returning `void`
```

Write no return type for a C `void` function. Keep `*mut c_void` for an untyped pointer.

Pass a slice as a pointer and a length. Pass a trait object behind an opaque handle that the
Rust side owns, as in the next section.

## Use a distinct opaque type, not `c_void`

`*mut c_void` accepts any pointer, so the compiler cannot tell one handle from another. A
zero-sized opaque struct gives each handle its own type and restores the type check:

```rust
use core::marker::{PhantomData, PhantomPinned};

#[repr(C)]
pub struct Handle {
    _data: [u8; 0],
    // Not Send, not Sync, not Unpin, and not constructible outside this module.
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

unsafe extern "C" {
    pub unsafe fn handle_new() -> *mut Handle;
    pub unsafe fn handle_free(handle: *mut Handle);
}
```

The `[u8; 0]` field makes the type zero-sized with a C-compatible layout. The `PhantomData`
field removes the auto traits, so the handle cannot be sent to another thread by accident.

Inside an `unsafe extern` block (1.82), mark a foreign function `safe fn` when it has no
preconditions, for example `pub safe fn handle_count() -> usize;`. A call to it then needs no
`unsafe` block. The `unsafe extern` itself is your promise that every signature in the block is
correct.

## Pass a closure to C as data plus a function pointer

C has no closures. Split the closure into a plain `extern "C"` function and a context pointer.
The function is the code, the context is the data.

```rust
use std::os::raw::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

unsafe extern "C" {
    unsafe fn c_register(
        callback: Option<unsafe extern "C" fn(i32, *mut c_void)>,
        context: *mut c_void,
    );
}

unsafe extern "C" fn trampoline<F: FnMut(i32)>(value: i32, context: *mut c_void) {
    if context.is_null() {
        return;
    }
    // SAFETY: `context` is the pointer that `register` leaked from a `Box<F>`.
    // The C library returns it unchanged and calls back on the registering
    // thread only, so no other reference to it exists during this call.
    let callback = unsafe { &mut *(context as *mut F) };
    // A panic must not unwind into C. The helper catches a second panic from a
    // hostile payload destructor. See the rust-panic-safety skill.
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| callback(value))) {
        discard_panic_payload(payload);
    }
}

pub fn register<F: FnMut(i32) + 'static>(callback: F) -> *mut c_void {
    let context = Box::into_raw(Box::new(callback)) as *mut c_void;
    // SAFETY: `trampoline::<F>` reads `context` as `*mut F`, which is the type
    // that was boxed. The box stays alive until `unregister` reclaims it.
    unsafe { c_register(Some(trampoline::<F>), context) };
    context
}

/// # Safety
/// `context` must be a pointer that `register::<F>` returned, with the same `F`,
/// and the C library must have stopped calling the callback.
pub unsafe fn unregister<F: FnMut(i32) + 'static>(context: *mut c_void) {
    // SAFETY: the caller guarantees the pointer came from `Box::into_raw` on a
    // `Box<F>` and that no callback is in flight.
    drop(unsafe { Box::from_raw(context as *mut F) });
}
```

The monomorphized `trampoline::<F>` is what makes this work: each `F` gets its own symbol with
the C ABI. Always provide the unregister path. Without it the box leaks, and the leak grows once
per registration.

Use `Option<unsafe extern "C" fn(..)>` for a nullable callback, not a raw pointer. A function
pointer is non-null, so `Option` of one is still a single word and it maps to a C null.

## Use `OwnedFd` and `BorrowedFd`, not `RawFd`

A `RawFd` is an `i32`. It says nothing about who closes it, so a double close or a
use-after-close is a plain integer bug that the compiler cannot see. The I/O safety types encode
the ownership:

```rust
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

// Borrow: the lifetime ties the descriptor to the owner.
pub fn borrow(file: &std::fs::File) -> BorrowedFd<'_> {
    file.as_fd()
}

// Own: closing is now the type's job.
pub fn own(file: std::fs::File) -> OwnedFd {
    OwnedFd::from(file)
}

/// # Safety
/// `fd` must be open, and no other object may own it. This function takes
/// ownership; the descriptor is closed when the returned value is dropped.
pub unsafe fn adopt(fd: i32) -> OwnedFd {
    // SAFETY: the caller guarantees sole ownership of an open descriptor.
    unsafe { OwnedFd::from_raw_fd(fd) }
}
```

Use `as_raw_fd` only at the call that hands the number to C, and keep the owner alive across
that call. Windows has the same three types under `std::os::windows::io` for handles and
sockets.

## Do not hand-roll bitfields

C bitfield layout is implementation defined. The bit order inside a storage unit, the straddling
rule, and the padding all vary by compiler and by target. A hand-written mask and shift pair
encodes one compiler's choice, and the mismatch appears as a corrupted field on a different
target.

Represent the storage unit as a plain integer and generate the accessors. `bitflags` covers flag
sets. `modular-bitfield` and `bitfield-struct` cover packed fields with explicit widths. Whatever
you pick, write a round-trip test against a byte vector captured from the real peer.

## Use `assert!`, not `debug_assert!`, to guard an unsafe block

`debug_assert!` disappears in a release build. A check that guards an unsafe block must survive
the profile that ships:

```rust
/// # Safety
/// `index` must be in bounds for `slice`.
pub unsafe fn get_unchecked_checked<T>(slice: &[T], index: usize) -> &T {
    assert!(index < slice.len(), "index {index} out of bounds");
    // SAFETY: the assertion above proved the index is in bounds.
    unsafe { slice.get_unchecked(index) }
}
```

Use `debug_assert!` only for an invariant that is already guaranteed by construction and that
you check to catch a refactor. If the check is what makes the unsafe block sound, it is not a
debug assertion.

## Attach `PhantomData` to a struct that holds raw pointers

A raw pointer carries no lifetime and gives the struct no drop check. A struct that holds one
therefore outlives the data it points at, and the compiler reports nothing. Variance is not part
of that loss. `*const T` is covariant in `T`, exactly like `&T`, and a struct whose only field is
`*const T` is covariant in `T` with no `PhantomData` at all. Only `*mut T` is invariant.

```rust
pub struct ConstHolder<T> { p: *const T }
pub struct MutHolder<T> { p: *mut T }

// Compiles: `ConstHolder<T>` is covariant in `T`, so `'b` shrinks to `'a`.
pub fn shrink<'a, 'b: 'a>(x: ConstHolder<&'b u8>) -> ConstHolder<&'a u8> { x }
```

The same function over `MutHolder` is rejected:

```text
error: lifetime may not live long enough
  = note: requirement occurs because of the type `MutHolder<&u8>`, which makes the generic argument `&u8` invariant
  = note: the struct `MutHolder<T>` is invariant over the parameter `T`
```

rustc names the struct here. A `*mut` that sits directly in the signature, with no struct around
it, gives the other pair of notes:

```text
  = note: requirement occurs because of a mutable pointer to `&u8`
  = note: mutable pointers are invariant over their type parameter
```

The two pairs never appear together. Grep a build log for the pair that matches the shape you
wrote.

What you must state with a marker is the lifetime and the ownership:

```rust
use core::marker::PhantomData;

// Behaves like `&'a T`: covariant in `T`, and `'a` is enforced.
pub struct Ref<'a, T> {
    ptr: *const T,
    _marker: PhantomData<&'a T>,
}

// Behaves like `T`: the drop checker knows a `T` may be dropped here.
pub struct Owned<T> {
    ptr: *mut T,
    _marker: PhantomData<T>,
}
```

### Pick the marker by the three properties it changes

`PhantomData<X>` makes the struct behave for variance, auto traits, and drop check as if it held
an `X`. Pick the `X` for an FFI handle on purpose, not from habit. The `rust-variance` skill, when
it is installed, owns the marker table for variance and auto traits, with the probes that measure
each row. For a handle, only `PhantomData<T>` declares that the handle owns a `T` for drop check;
`PhantomData<&'a T>`, `PhantomData<*const T>`, and `PhantomData<*mut T>` do not.

To strip `Send` and `Sync` from a handle, use `PhantomData<*const T>`. It removes both auto
traits exactly as `PhantomData<*mut T>` does, and it stays covariant in `T`. Reach for
`PhantomData<*mut T>` only when you also want invariance, which is the case where `T` is written
through the handle. Choosing `*mut` by reflex freezes the type parameter for every caller of your
API, and nothing in the code says why.

```rust
use core::marker::PhantomData;

// Not `Send`, not `Sync`, still covariant in `T`.
pub struct Handle<T> { raw: usize, _marker: PhantomData<*const T> }

// Not `Send`, not `Sync`, and invariant in `T`. Pick this only if you want that.
pub struct WriteHandle<T> { raw: usize, _marker: PhantomData<*mut T> }
```

### `PhantomData<T>` buys no drop check next to a plain `Drop` impl

On stable, a plain `impl Drop` already forces every type parameter to strictly outlive the value,
so `PhantomData<T>` there buys variance and auto traits, not drop check. An owning FFI handle
needs a `Drop` impl to free the resource, so this is the usual case. The marker becomes
load-bearing for drop check next to `unsafe impl<#[may_dangle] T> Drop`, which needs nightly and
`#![feature(dropck_eyepatch)]`. On stable the attribute is
`error[E0658]: may_dangle has unstable semantics and may be removed in the future`. The
`rust-variance` skill owns the one stable case where the marker does change drop check: a struct
with drop glue but no `Drop` impl of its own.

The stable consequence is a trap in the other direction. `Vec<T>` and `Box<T>` carry the
eyepatch. A newtype over one of them does not, so adding a `Drop` impl to the newtype breaks
code that compiled before, even when the body of `drop` is empty:

```rust,compile_fail,E0597
struct Wrapper<T>(Vec<T>);
impl<T> Drop for Wrapper<T> { fn drop(&mut self) {} }   // delete this and it compiles

fn main() {
    let mut w: Wrapper<&String> = Wrapper(Vec::new());
    let s = String::new();
    w.0.push(&s);
}
```

```text
error[E0597]: `s` does not live long enough
   | `s` dropped here while still borrowed
   | borrow might be used here, when `w` is dropped and runs the `Drop` code for type `Wrapper`
```

You cannot recover the relaxation on stable. Do not add a `Drop` impl you do not need.

## Restrict `union` to C interop

A Rust `union` has no active field. A read interprets the shared storage as the selected
field's type. Prove that the storage meets that type's initialization and validity
requirements, including pointer provenance, lifetime, and aliasing. Writing one
field and reading another is permitted when that proof holds. `#[repr(C)]` puts each field
at offset zero; the default representation does not guarantee that layout.

```rust,run
#[repr(C)]
union Bits { integer: u32, float: f32 }

fn main() {
    let bits = Bits { integer: 0x3f80_0000 };
    // SAFETY: repr(C) overlays both four-byte fields. All bytes are initialized,
    // and every u32 bit pattern represents a valid f32.
    assert_eq!(unsafe { bits.float }, 1.0);
}
```

This probe checks a valid numeric reinterpretation. It does not prove a pointer or reference
reinterpretation sound. Source: [Rust Reference, union reads](https://doc.rust-lang.org/reference/items/unions.html#reading-and-writing-union-fields).

- Use a `union` only to match a C type that is itself a union.
- Do not put a reference, `NonNull`, `Box`, or other type with a validity invariant beside an
  integer variant. Borrowck tracks the lifetime of a `&'a T` field (rustc 1.99.1 rejects a
  dangling one with E0597), but a read of `r` after a write of `n` fabricates a reference from an
  arbitrary integer, which is undefined behavior. Store a raw pointer when the C type holds a
  pointer.
- Pair every union with the discriminant that the C API uses, and read the discriminant first.
- Prefer an `enum` whenever the layout is yours to choose. It carries the tag for you.

A field type that implements `Drop` is rejected outright, because the union has no tag and the
compiler cannot emit drop glue for it:

```text
error[E0740]: field must implement `Copy` or be wrapped in `ManuallyDrop<...>` to be used in a union
  = note: union fields must not have drop side-effects, which is currently enforced via either
          `Copy` or `ManuallyDrop<...>`
```

Adding `impl Drop` to a type therefore bans it retroactively from every union in the workspace
that holds it. The fix rustc suggests, `ManuallyDrop<...>`, moves the destructor obligation into
your unsafe code: you must then call `ManuallyDrop::drop` on the live variant yourself, exactly
once, after reading the discriminant.

## Symbol collision in `cdylib` crates

Edition 2024 requires `#[unsafe(no_mangle)]`, `#[unsafe(export_name = "...")]`, and
`#[unsafe(link_section = "...")]`. The `unsafe` marks an old hazard: when two compilation units
export the same unmangled symbol, the linker silently picks one, and the wrong function runs.

Since 1.98, rustc checks a definition of a core runtime symbol such as `memset`, `memcpy`, or
`strlen`, with two lints. A wrong number of parameters is an error
(`invalid_runtime_symbol_definitions`, deny by default). A wrong parameter or return type with
the right count is only a warning (`suspicious_runtime_symbol_definitions`, warn by default):
`extern "C" fn strlen(_: u32) -> u8` builds, and fails only under `-D warnings` (Rust 1.99.1).
Every other name still collides with no diagnostic. The JNI convention,
`Java_<package>_<class>_<method>`, gives natural uniqueness. Check every other unmangled export
for uniqueness across all native libraries that the host process can load at the same time,
including libraries you do not own. The third inventory command in [SKILL.md](../SKILL.md) lists
every unmangled export.

## Zero-copy buffer handoff

A slice built over a foreign caller's buffer without a copy needs three caller guarantees for the
whole call. State them in the SAFETY comment and again at the foreign call site:

1. The pointer is non-null and aligned for the element type.
2. Access is exclusive: the caller does not read or write concurrently.
3. The buffer stays valid for the entire call.

For a slice over a memory mapping, the mapping must outlive the slice, and nothing may mutate
the region while the slice is live. An output lifetime that appears in no argument is unbounded:
the caller picks it, up to `'static`, and nothing checks the choice. std's
`slice::from_raw_parts` has this shape and makes the caller bound it by annotation. Prefer to tie
it to an input. Otherwise keep such a function private, and make the owning type hold the
mapping so that the borrow checker enforces the relationship. Read
[A slice over a caller-owned buffer or a mapping](#a-slice-over-a-caller-owned-buffer-or-a-mapping) when you build such a slice.

## Related

- [unsafe-patterns.md](unsafe-patterns.md) — safety comments, FFI entry points, and transmute
- [miri-and-aliasing.md](miri-and-aliasing.md) — auto traits, aliasing models, Miri, and clippy
- [SKILL.md](../SKILL.md) — the lint floor and the FFI panic facts
- [audit-checklist.md](audit-checklist.md) — the review checklist

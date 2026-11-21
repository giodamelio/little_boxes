# Unsafe patterns reference

Worked patterns for the rules in [SKILL.md](../SKILL.md). Copy the shape, not the type names.

Contents:

- Safety comment format; unbounded output lifetimes
- `unsafe trait` and `unsafe fn` axes
- The five unsafe operations; raw pointers to statics and union fields
- FFI entry points: repeated JNI exports, hand-rolled `extern "C"`, a JNI method export;
  `from_raw` handles
- Wrapping a caller-owned buffer
- Syscall, ioctl, union, and descriptor wrappers
- `ptr::read`, `ptr::write`, `copy_nonoverlapping`; unaligned reads from untrusted bytes;
  `align_to`
- Pointer arithmetic and `NonNull`
- Transmute table and `dyn` lifetime extension; broken UTF-8; `black_box`
- Drop hazards; explicit close and the limits of generic cleanup

## Safety comment format

Every `unsafe { ... }` block needs a `// SAFETY:` comment above it. Every `unsafe fn` needs a
`/// # Safety` rustdoc section that lists what the caller must guarantee. The two are different
documents: the rustdoc states the contract, the inline comment states why this call site meets
it.

```rust
/// # Safety
/// `ptr` must be non-null, aligned to `align_of::<T>()`, and point at `len`
/// initialized values of type `T`. Their total byte size must not exceed
/// `isize::MAX`. The memory must stay valid and must not be mutated for the
/// lifetime `'a` of the returned slice.
unsafe fn raw_slice<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    let element_size = std::mem::size_of::<T>();
    assert!(element_size == 0 || len <= isize::MAX as usize / element_size);
    // SAFETY: the caller guarantees `ptr` is non-null, aligned, and valid for
    // `len` initialized elements.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}
```

The inner `unsafe` block is required even inside an `unsafe fn`, because
`#![deny(unsafe_op_in_unsafe_fn)]` is part of the lint floor. Without it, an `unsafe fn` becomes
a region where every operation is implicitly permitted and no operation is individually
justified.

## An output lifetime that appears in no input is unbounded

`raw_slice` above returns `&'a [T]`, and `'a` appears in no argument. The caller picks `'a`, and
the caller may pick `'static`. Nothing warns. Lifetime inference solves for the output from the
constraints in the signature, and a raw pointer carries none, so there is no upper bound.
`<*const T>::as_ref`, which returns `Option<&'a T>`, has the same shape and the same hazard.

```rust
/// # Safety
/// `p` must be valid for reads for all of `'a`. The caller must bound `'a`
/// itself, and nothing checks that it did.
pub unsafe fn deref_unbounded<'a, T>(p: *const T) -> &'a T {
    // SAFETY: the caller bounds `'a`; nothing enforces it.
    unsafe { &*p }
}
```

That compiles clean. A caller that omits the bound gets a reference that outlives its owner, and
nothing reports it. Tie the output lifetime to an input the caller must already hold:

```rust
/// # Safety
/// `p` must point into `_owner`, and must stay valid for all of `'a`.
pub unsafe fn deref_bounded<'a, T, O>(p: *const T, _owner: &'a O) -> &'a T {
    // SAFETY: the caller guarantees `p` points into `_owner`, which lives for `'a`.
    unsafe { &*p }
}
```

An escape is now `error[E0597]: ... does not live long enough`, reported at the call site, with
`borrowed value does not live long enough` on the owner argument.

Marking the function `unsafe` does not repair the shape. `unsafe` moves the obligation to the
caller, and the compiler cannot check how the caller discharges it. std's `slice::from_raw_parts`
has this shape, and its documentation tells the caller to bound the lifetime by explicit
annotation or with a helper that takes the owner. When the value is reachable from something the
caller already names, take `&'a self` or a `&'a Owner` parameter and make the function safe. When
a trait must produce a borrowed value, use a generic associated type, which puts the lifetime
back into the signature:

```rust
pub struct Store;

pub trait Extract {
    type Out<'w>;
    fn from_store<'w>(store: &'w Store) -> Self::Out<'w>;
}
```

Keep an unbounded output lifetime only where the SAFETY comment names the owner, the function is
private, and the owning type holds the data, as in the `mmap_as_slice` case in
[ffi-layout-rules.md](ffi-layout-rules.md).

## `unsafe trait` and `unsafe fn` are separate axes

`unsafe` on a trait and `unsafe` on a method constrain different people. Neither one implies the
other.

| Declaration | Who carries the obligation | Compiler rule |
| --- | --- | --- |
| `unsafe trait T` | The implementor | A plain `impl T for X` is `error[E0200]: the trait T requires an unsafe impl declaration` |
| A plain `trait T` | Nobody beyond the type system | `unsafe impl T for X` is `error[E0199]: implementing the trait T is not unsafe` |
| `unsafe fn m()` in a trait | The caller | The call site needs an `unsafe { ... }` block |
| A safe `fn m()` in an `unsafe trait` | Nobody at the call site | The call needs no `unsafe` block |

Do not write the rule as "an `unsafe trait` makes its methods unsafe to call". A safe method of
an `unsafe trait` is called with no block. The mirror trap costs as much: `unsafe impl` on a
safe trait is E0199, so you cannot use the keyword to signal that an impl is delicate.

Put the `# Safety` section where the obligation sits. An invariant that the implementor must
uphold belongs on the trait. An invariant that the caller must uphold belongs on the method. A
trait can carry both.

```rust
/// # Safety
/// The implementor must keep `len()` equal to the initialized prefix.
unsafe trait RawView {
    fn len(&self) -> usize; // Safe to call. No unsafe block at the call site.
    /// # Safety
    /// `i` must be less than `self.len()`.
    unsafe fn at(&self, i: usize) -> u8;
}

struct Buf(Vec<u8>);

// SAFETY: `Buf` owns a fully initialized `Vec`, so `len()` is exact.
unsafe impl RawView for Buf {
    fn len(&self) -> usize { self.0.len() }
    unsafe fn at(&self, i: usize) -> u8 {
        // SAFETY: the caller guarantees `i < self.len()`.
        unsafe { *self.0.get_unchecked(i) }
    }
}

fn main() {
    let buf = Buf(vec![1, 2, 3]);
    let n = buf.len();               // No unsafe block: the method is safe.
    let byte = unsafe { buf.at(1) }; // SAFETY: 1 < 3.
    assert_eq!((n, byte), (3, 2));
}
```

## The five unsafe operations

An `unsafe` block permits five operations. Nothing else changes, and the borrow checker keeps
running.

1. Dereference a raw pointer (`*const T`, `*mut T`).
2. Call an unsafe function. This includes a foreign function that is not declared `safe fn`, and
   a `#[target_feature]` function when the caller does not enable the same features.
3. Read or write a `static mut` or an unsafe `extern` static.
4. Read a field of a union.
5. Execute inline assembly.

If a block does none of these, delete it. An `unsafe impl`, an unsafe attribute, and an `unsafe
extern` block are declarations, not operations; each needs its own review and its own SAFETY
comment.

## Raw pointers to statics and union fields

Taking a raw pointer needs no `unsafe` block. Only the access through it does. This probe
compiles and runs with no `unsafe` around either `&raw` expression:

```rust,run
static mut COUNTER: u32 = 0;

#[repr(C)]
union Bits {
    int: u32,
    float: f32,
}

fn main() {
    // Safe since 1.82 for a static, and since 1.92 for a union field.
    let counter: *mut u32 = &raw mut COUNTER;
    let bits = Bits { int: 0x3f80_0000 };
    let float: *const f32 = &raw const bits.float;

    // SAFETY: one thread, and no reference to `COUNTER` exists.
    unsafe { *counter += 1 };
    // SAFETY: as above.
    let count = unsafe { *counter };
    // SAFETY: repr(C) overlays both fields. `bits` is live and initialized,
    // and every `u32` bit pattern is a valid `f32`.
    let value = unsafe { *float };
    assert_eq!(count, 1);
    assert_eq!(value, 1.0);
}
```

## Repeated JNI exports

On `jni` 0.22, do not stamp `Java_*` exports by hand. Bind with `native_method!` plus
`RegisterNatives`, which also generates the panic guard, or with `#[jni_mangle]`. The `rust-jni`
skill, when it is installed, has both patterns. Keep a `macro_rules!` stamp only for a family of
plain `extern "C"` exports, and put the `catch_unwind` guard inside the macro body, not only in
the delegated function.

## A hand-rolled `extern "C"` entry point

Use this only when a generated boundary is not an option. Every pointer argument gets its own
SAFETY comment, because each carries a different invariant. The `catch_unwind` turns a panic into
a status for a caller that needs one; without it, a panic aborts the process at the boundary.
`discard_panic_payload` is the payload-disposal helper from the `rust-panic-safety` skill: it
drops the payload without letting a panicking payload destructor escape.

```rust
/// # Safety
/// `pixel_ptr` must be non-null, aligned for 4 bytes, writable, exclusively owned
/// by the caller for the duration of this call, and valid for `width * height * 4`
/// bytes. `spec` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_into_buffer(
    pixel_ptr: *mut u8,
    width: u32,
    height: u32,
    spec: *const std::os::raw::c_char,
) -> i32 {
    let Some(pixel_len) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|length| *length <= isize::MAX as usize)
    else {
        return -1;
    };
    if pixel_ptr.is_null() || spec.is_null() {
        return -1;
    }
    let result = std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees `pixel_ptr` is valid for
        // `pixel_len` bytes, writable, and not aliased. The checks above reject
        // null pointers and arithmetic overflow.
        let pixels = unsafe { std::slice::from_raw_parts_mut(pixel_ptr, pixel_len) };
        // SAFETY: the caller guarantees `spec` is a valid null-terminated C string.
        let spec = unsafe { std::ffi::CStr::from_ptr(spec) }
            .to_str()
            .map_err(|_| Error::InvalidRequest)?;
        render_pixels(pixels, width, height, spec)
    });
    match result {
        Ok(Ok(())) => 0,
        Ok(Err(_)) => -1,
        Err(payload) => {
            discard_panic_payload(payload);
            -2
        }
    }
}
```

Return a plain integer status. Do not return a `Result`, a `String`, or any type whose layout
the foreign caller cannot rely on.

## A JNI method export

For a JNI method export, guard with `EnvUnowned::with_env` and exit through `resolve`. The
`rust-panic-safety` skill, when it is installed, owns the pattern and the `jni` 0.21 variant, and
the `rust-jni` skill, when it is installed, owns `JNI_OnLoad`.

## Taking ownership of a raw FFI handle

A `from_raw` constructor takes ownership of a raw handle that crossed the FFI boundary. State
three invariants in the `// SAFETY:` comment above the call:

- The raw value came from the matching `into_raw` or `into_raw_fd`, or the foreign API documents
  that it transfers ownership, and it is still live.
- Nothing else owns it. Call `from_raw` exactly once, and make sure no other code frees or
  closes it. A second call is a double free or a double close.
- If the foreign side only lends the handle for one call or one frame (for example a JNI local
  reference or a caller's descriptor), do not take ownership. Use a borrowing constructor such as
  `BorrowedFd::borrow_raw`, and do not let the value outlive that frame.

Call an owning constructor, such as `Box::from_raw`, `Arc::from_raw`, or
`OwnedFd::from_raw_fd`, in one module only. Wrap the result in a type that the rest of the
workspace clones or borrows, and write the liveness argument once there. A grep for the
constructor name then finds a second `from_raw` call in review.

On `jni` 0.22, `JavaVM::from_raw` returns a clone of a process singleton, so it has none of these
ownership invariants; only the non-null assertion applies.

## Wrapping a caller-owned buffer in a library object

Some C and C++ libraries render into a buffer that the caller allocated. The wrapper object must
not outlive the buffer, and nothing else may touch the buffer while the object exists. Encode
both rules with a lifetime: the wrapper holds a `PhantomData<&'buf mut [u8]>`, so the borrow
checker enforces them and the constructor stays a safe function.

Before you write this wrapper, read the signature of the binding crate's own wrap function. A
safe binding often returns a borrowing type already. Calling a safe function inside `unsafe`
only produces an `unused_unsafe` warning, and returning an owned type from it discards the
lifetime that the binding enforces.

```rust
use std::marker::PhantomData;
use std::ptr::NonNull;

#[repr(C)]
pub struct RawSurface {
    _private: [u8; 0],
}

unsafe extern "C" {
    /// Renders into `pixels` without copying, until `surface_destroy`.
    fn surface_wrap(pixels: *mut u8, width: i32, height: i32, row_bytes: usize) -> *mut RawSurface;
    fn surface_destroy(surface: *mut RawSurface);
}

/// A surface that renders into a buffer the caller owns.
pub struct Surface<'buf> {
    raw: NonNull<RawSurface>,
    _pixels: PhantomData<&'buf mut [u8]>,
}

impl<'buf> Surface<'buf> {
    pub fn wrap(pixels: &'buf mut [u8], width: i32, height: i32) -> Option<Self> {
        let width_usize = usize::try_from(width).ok()?;
        let height_usize = usize::try_from(height).ok()?;
        let row_bytes = width_usize.checked_mul(4)?;
        let required_bytes = row_bytes.checked_mul(height_usize)?;
        if width_usize == 0 || height_usize == 0 || pixels.len() < required_bytes {
            return None;
        }
        // SAFETY: the checked calculations prove that `pixels` holds `height` rows of
        // `row_bytes` bytes. The `'buf` borrow keeps the buffer alive and exclusive
        // until `Drop` destroys the surface.
        let raw = unsafe { surface_wrap(pixels.as_mut_ptr(), width, height, row_bytes) };
        NonNull::new(raw).map(|raw| Surface { raw, _pixels: PhantomData })
    }
}

impl Drop for Surface<'_> {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `surface_wrap`, and `Drop` runs exactly once.
        unsafe { surface_destroy(self.raw.as_ptr()) }
    }
}
```

When the library hands back a view into memory that it owns, copy the bytes out immediately
through the binding's safe byte slice. Do not reconstruct a slice from a caller-supplied height.
Do not store the view or let it escape the scope where the owning object is alive. The
binding computes the byte extent of the view from its own image information and row stride. A
copy into an owned `Vec` costs one memcpy and removes the raw-pointer, length, and lifetime
obligations.

## Syscall, ioctl, union, and descriptor wrappers

Give every syscall wrapper a `# Safety` section that lists the descriptor-validity and
layout-match invariants. Check the return value and convert `io::Error::last_os_error()`; never
discard `errno`.

Use `mem::zeroed()` only when the exact type documents that all-zero bytes are a
valid value. `repr(C)` controls layout. It does not remove Rust value invariants.

```rust
// SAFETY: every field of `ifreq` is an integer, an integer array, a struct of
// integers, or a raw pointer, directly or inside the `ifr_ifru` union; zero is
// valid for each (null for the pointer). The name is written below.
let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
ifr.ifr_name = make_ifr_name();
```

```rust
use std::io;

/// # Safety
/// `fd` must be a live socket descriptor. `T` must match the layout the kernel
/// writes for the given `level` and `name` pair. On success, the option must
/// initialize exactly `size_of::<T>()` bytes with a valid value of `T`.
unsafe fn getsockopt_raw<T>(
    fd: libc::c_int,
    level: libc::c_int,
    name: libc::c_int,
) -> io::Result<(T, libc::socklen_t)> {
    let mut val = std::mem::MaybeUninit::<T>::uninit();
    let mut len = std::mem::size_of::<T>() as libc::socklen_t;
    // SAFETY: `fd` is live per the caller contract; `val` and `len` are valid
    // for writes of the sizes passed.
    let rc = unsafe { libc::getsockopt(fd, level, name, val.as_mut_ptr().cast(), &mut len) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    if len as usize != std::mem::size_of::<T>() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "short socket option"));
    }
    // SAFETY: the caller contract requires a fully initialized, valid `T`.
    Ok((unsafe { val.assume_init() }, len))
}
```

An `ioctl` SAFETY comment states three facts: the descriptor is valid, the struct fields the
kernel reads are populated, and which request number is issued and what it does.

```rust
// SAFETY: `sock` is a valid AF_INET/SOCK_DGRAM descriptor; `ifr` has `ifr_name`
// and the MTU field set; SIOCSIFMTU sets the interface MTU.
let rc = unsafe { libc::ioctl(sock.as_raw_fd(), libc::SIOCSIFMTU, &ifr as *const _) };
if rc < 0 {
    return Err(Error::Ioctl("SIOCSIFMTU", io::Error::last_os_error()));
}
```

A C union field read is unsafe because the compiler cannot know which variant was written last.
A write to a `Copy` union field is safe, so it takes no `unsafe` block; an `unsafe` block around
it only produces an `unused_unsafe` warning. Zero the struct first, write the field, and put the
`unsafe` block and its SAFETY comment on the read. Match the field type: the `libc` flag
constants are `c_int`, and `ifru_flags` is `c_short`.

```rust
fn tun_flags() -> libc::c_short {
    // SAFETY: `ifreq` is plain C data, and all-zero bytes are a valid `ifreq`.
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    ifr.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as libc::c_short;
    // SAFETY: `ifru_flags` is the union field written last.
    unsafe { ifr.ifr_ifru.ifru_flags }
}
```

A descriptor that a foreign caller passes in is borrowed, not owned. Duplicate it before you
take ownership, or the foreign runtime closes it under you. The standard library does this with
no dependency:

```rust
use std::os::fd::{BorrowedFd, OwnedFd};

fn adopt_copy(borrowed: BorrowedFd<'_>) -> std::io::Result<OwnedFd> {
    borrowed.try_clone_to_owned()
}
```

Call this with `owner.as_fd()` while the owner is borrowed. At a raw FFI entry point,
construct `BorrowedFd` only after establishing that the descriptor stays open throughout
the borrow. A helper that accepts `RawFd` and requires this caller guarantee is `unsafe fn`
with a `# Safety` contract; a safe signature cannot impose it. See
[`BorrowedFd::borrow_raw`](https://doc.rust-lang.org/1.100.0/std/os/fd/struct.BorrowedFd.html#method.borrow_raw).

`nix::unistd::dup` changed its signature in nix 0.30: it now takes `impl AsFd` and returns an
`OwnedFd`, so code written for 0.29 does not compile against the current release.

## Reads and writes that create no reference

These three functions move values through a raw pointer without ever forming a `&` or `&mut`,
so they do not interact with the aliasing model the way a reference does. That is exactly why
they are the right tool for partially initialized or externally owned memory.

```rust
// ptr::read performs a bitwise copy and leaves the source bytes in place.
// SAFETY: `ptr` is valid, aligned, and initialized. The source is now treated
// as moved-from and will not be read or dropped again unless `T: Copy`.
let val: u32 = unsafe { std::ptr::read(ptr) };

// ptr::write stores `T` without dropping whatever was there before.
// SAFETY: `dst` is valid for writes and aligned; the previous value, if any,
// was already moved out or is not initialized.
unsafe { std::ptr::write(dst, new_val) };

// ptr::copy_nonoverlapping is memcpy. Overlap is UB; use ptr::copy for memmove.
// SAFETY: `src` and `dst` are valid for `count` elements and do not overlap.
// For non-Copy `T`, ownership accounting guarantees that exactly one copy of
// each value is later dropped.
unsafe { std::ptr::copy_nonoverlapping(src, dst, count) };
```

`ptr::read` on a byte slice from I/O needs `ptr::read_unaligned` instead, as the next section
shows. The rules that govern it are in [SKILL.md](../SKILL.md).

## Unaligned reads from untrusted bytes

Four forms of the same parse, worst first, then a bulk read. Each one is a whole example, because
only the first is wrong and a reader must be able to copy the others without carrying the mistake
along.

The bad form compiles, runs, and returns the right bytes on an x86-64 development host, because
x86-64 and AArch64 user space perform most misaligned loads. The optimizer still relies on the
promised alignment. Atomic and exclusive accesses on AArch64, multi-word loads on 32-bit ARM, and
some RISC-V cores trap or take a slow emulation path. Only cargo-careful and Miri report it. Default
Miri reports it on some seeds only (on Miri nightly-2026-11-09, a `vec![0u8; 16]` buffer failed on
15 of seeds 0 to 15 and passed on seed 10); `-Zmiri-symbolic-alignment-check` reports it on every seed:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
struct Header { magic: u32, len: u32 }

fn parse(buf: &[u8]) -> Header {
    // BAD: `read` requires an aligned pointer, and a byte slice promises nothing.
    unsafe { std::ptr::read(buf.as_ptr() as *const Header) }
}
```

The correct form states that the read tolerates misalignment, and checks the length first:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
struct Header { magic: u32, len: u32 }

fn parse(buf: &[u8]) -> Option<Header> {
    if buf.len() < std::mem::size_of::<Header>() {
        return None;
    }
    // SAFETY: the length is checked, and `read_unaligned` needs no alignment.
    Some(unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const Header) })
}
```

The better form removes the `unsafe` block. `zerocopy` proves the layout at compile time and
returns the remaining bytes with the value:

```rust
use zerocopy::{FromBytes, Immutable, KnownLayout};

#[derive(FromBytes, KnownLayout, Immutable, Clone, Copy)]
#[repr(C)]
struct Header { magic: u32, len: u32 }

fn parse(buf: &[u8]) -> Option<Header> {
    let (header, _rest) = Header::read_from_prefix(buf).ok()?;
    Some(header)
}
```

A streaming parser reads field by field with an explicit endianness, and never casts a pointer:

```rust
use bytes::Buf;

fn magic(buf: &[u8]) -> Option<u32> {
    let mut cursor = std::io::Cursor::new(buf);
    if cursor.remaining() < 4 {
        return None;
    }
    Some(cursor.get_u32_le())
}
```

For a bulk read, `align_to` splits a byte slice into an unaligned head, an aligned middle, and an
unaligned tail:

```rust
pub fn aligned_middle(bytes: &[u8]) -> usize {
    // SAFETY: `u32` has no invalid bit pattern and no padding, so reinterpreting
    // aligned bytes as `u32` is valid.
    let (_head, middle, _tail) = unsafe { bytes.align_to::<u32>() };
    middle.len()
}
```

Never use `align_to` for a type that has an invalid bit pattern. `bool`, `char`, a `NonZero`
type, and every enum have invalid bit patterns. Reinterpreting arbitrary bytes as one of them is
undefined behavior even when the alignment is correct.

Enable `clippy::cast_ptr_alignment`. It catches the `as *const u32` form. It does not catch every
case, so the rule stands on its own.

## Pointer arithmetic

```rust
// wrapping_add: always sound to compute. Do not dereference an out-of-bounds result.
let p = ptr.wrapping_add(2);

// add: UB when the result leaves the allocation, even without a dereference.
// SAFETY: `ptr + 2` stays inside the same allocation.
let third = unsafe { *ptr.add(2) };

// offset_from: both pointers must belong to the same allocation. It returns `isize`.
// SAFETY: `end` and `start` point into the same contiguous slice.
let signed = unsafe { end.offset_from(start) };

// offset_from_unsigned (1.87): the same, and `end >= start` is also a precondition.
// Use it instead of an `as usize` cast of `offset_from`.
// SAFETY: as above, and `end` is not before `start`.
let count: usize = unsafe { end.offset_from_unsigned(start) };
```

`add` is UB on computation, not on dereference. This surprises people: computing a pointer one
past the end of an allocation is allowed, computing two past is not.

`add`, `offset`, and `wrapping_add` return a new pointer and never modify the receiver, so a
cursor translated from C's `p++` must be assigned back: `self.ptr = unsafe { self.ptr.add(1) };`.
Dropping the assignment compiles, and the cursor never advances. The only signal is a warning:

```text
warning: unused return value of `std::ptr::mut_ptr::<impl *mut T>::add` that must be used
  = note: returns a new pointer rather than modifying its argument
```

`NonNull` documents the non-null invariant in the type instead of in a comment:

```rust
use std::ptr::NonNull;

let boxed = Box::new(Buffer::new());
let nn: NonNull<Buffer> = NonNull::new(Box::into_raw(boxed)).expect("Box::into_raw is non-null");
// SAFETY: `nn` came from a live Box and has not been freed.
let borrowed = unsafe { nn.as_ref() };
// SAFETY: `nn` still points at the allocation from Box::into_raw, consumed once.
let owned_again = unsafe { Box::from_raw(nn.as_ptr()) };
```

## Transmute safety table

| From | To | Sound? | Use instead |
| --- | --- | --- | --- |
| `u32` | `f32` | Yes, same size | `f32::from_bits(u)` |
| `[u8; 4]` | `u32` | Yes | `u32::from_ne_bytes(arr)` |
| `&T` | `*const T` | Yes | `std::ptr::from_ref(r)` (1.76) |
| `*mut T` | `*const T` | Yes | `p.cast_const()` (1.65) |
| `Box<T>` | `*mut T` | Yes | `Box::into_raw(b)` |
| `&'a T` | `&'b T`, longer lifetime | **No** | Restructure the lifetimes |
| `u8` | `bool` | **No**, unless 0 or 1 | Match on the value |
| `u8` | `MyEnum` | **No**, unless a valid tag | `MyEnum::try_from(u)` |
| `Vec<T>` | `Vec<U>` | **No** | Convert element by element |
| `&[u8]` | `&[Header]` | **No** | `zerocopy::FromBytes` |

Every "Yes" row has a named function. Use the function: it cannot be applied to the wrong pair
of types, and it survives a refactor that changes one of them.

To extend the lifetime bound of a `dyn` raw pointer, first try to keep `'a` in the stored type.
Transmute only when no method of the trait has a `where Self: 'x` bound and the owner outlives
every use of the pointer, and state both facts in the SAFETY comment (the `rust-variance`
skill, when it is installed, owns this rule).

## Broken UTF-8 fails nowhere near where you built it

`str::from_utf8_unchecked` is guarded for one case only: a byte-string literal. The
deny-by-default lint `invalid_from_utf8_unchecked` catches that case.

```text
error: calls to `std::str::from_utf8_unchecked` with an invalid literal are undefined behavior
  |                        the literal was valid UTF-8 up to the 1 bytes
  = note: `#[deny(invalid_from_utf8_unchecked)]` on by default
```

For bytes produced at run time nothing checks anything: not the lint, not
`-C debug-assertions=on`, and not Miri. UTF-8 validity is a library invariant, not a language
validity invariant, so Miri's abstract machine sees nothing wrong at construction. A clean Miri
run over the code that builds the `&str` proves nothing about that `&str`.

The undefined behavior surfaces in the consumer, and only in one kind of consumer:

```rust
// `v` ends with a lone 0xF0, so `s` is not valid UTF-8.
let s: &str = unsafe { std::str::from_utf8_unchecked(&v) };

let _ = s.replace("a", "b");   // no Miri fault: a byte-wise search, it never decodes
let _ = s.chars().count();     // no Miri fault: counts non-continuation bytes
for _c in s.chars() {}         // Miri reports UB: this is the only one that decodes
```

"No Miri fault" is not "sound". The `str` invariant is broken from the
`from_utf8_unchecked` call on, so every use of `s` is a bug. These two calls only do not fault
on the std of Miri nightly-2026-11-09, and a later std can decode in either one.

`Chars::count` delegates to `core`'s `count_chars`, which counts non-continuation bytes and never
builds a `char`. Only `Chars::next` reaches the decoder:

```text
error: Undefined Behavior: entering unreachable code
```

Measured on Miri nightly-2026-11-09. The error points into `core`'s UTF-8 decoder, not into the
code that built the `&str`.

Write the reproducing test as a decode loop. A test that ends in `.chars().count()` passes and
reports that the buffer is fine.

### `black_box` is not a soundness argument

Never justify an `unsafe` block with "`std::hint::black_box` stops the compiler from optimizing
the check away". The std documentation says `black_box` is provided on a best-effort basis and
"must not be relied upon to control critical program behavior". An argument that depends on an
optimizer failing to notice something is not an argument.

The shape appears when code validates bytes and then reuses the buffer:

```rust
// Do not do this. The unsafe block buys nothing over the safe call.
let s = match String::from_utf8_lossy(std::hint::black_box(&v)) {
    Cow::Owned(s) => s,
    Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(v) },
};
```

```rust
// Do this. One validation pass, no unsafe, and the bad input is reported.
let s = String::from_utf8(v)?;
```

`String::from_utf8` runs the same single scan, reuses the same buffer with no copy, and its
`FromUtf8Error` carries `utf8_error().valid_up_to()` so you can report the offending offset.

## Drop hazards

Soundness must not depend on `Drop` running. Safe code skips a destructor with `mem::forget`,
`Box::leak`, `ManuallyDrop::new`, an `Rc` or `Arc` cycle, or `process::exit`. So an API whose
soundness needs a guard's destructor is unsound. For example, if only a `Drop` unregisters a
pointer to borrowed data that foreign code holds, a forgotten guard leaves that pointer dangling.
Make a forgotten guard impossible or harmless, as `thread::scope` does: it keeps the scope inside
a closure and joins every thread itself. A future inside `select!` can be dropped at any
`.await`, so its correctness must not depend on its `Drop` either.

### Explicit close and the limits of generic cleanup

A panic inside `drop()` during an unwind aborts the process with `panic in a destructor during
cleanup`, and an outer `catch_unwind` cannot catch that double panic. Put fallible flushes in
an explicit `close()`. An arbitrary `Write::flush`, logging callback, or `W::drop` can block
or panic; removing `unwrap` does not make generic cleanup nonblocking or panic-free.

```rust
// DANGEROUS: aborts the process when dropped during an unwind.
impl Drop for BufferedWriter {
    fn drop(&mut self) {
        self.flush().unwrap();
    }
}
```

```rust,run
use std::cell::Cell;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

pub struct BufferedWriter<W: Write> {
    inner: W,
}

impl<W: Write> BufferedWriter<W> {
    pub fn close(mut self) -> io::Result<W> {
        self.inner.flush()?;
        Ok(self.inner)
    }
}

// No custom Drop: this wrapper never starts another flush during cleanup.
enum Mode { Ok, Error, Panic }
struct Probe { calls: Rc<Cell<usize>>, mode: Mode }
impl Write for Probe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> {
        self.calls.set(self.calls.get() + 1);
        match self.mode {
            Mode::Ok => Ok(()),
            Mode::Error => Err(io::Error::other("flush failed")),
            Mode::Panic => panic!("flush panic probe"),
        }
    }
}

fn main() {
    let calls = Rc::new(Cell::new(0));
    let make = |mode| BufferedWriter { inner: Probe { calls: calls.clone(), mode } };
    drop(make(Mode::Ok));
    assert_eq!(calls.get(), 0);
    let writer = make(Mode::Ok).close().unwrap();
    drop(writer);
    assert_eq!(calls.get(), 1);
    assert!(make(Mode::Error).close().is_err());
    assert_eq!(calls.get(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = make(Mode::Panic).close();
    })).is_err());
    assert_eq!(calls.get(), 3); // No second flush while close unwinds.
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _writer = make(Mode::Panic);
        panic!("outer panic probe");
    })).is_err());
    assert_eq!(calls.get(), 3); // The wrapper does not flush during unwind.
}
```

Run this probe on an unwinding profile. It checks that this wrapper calls `flush` only from
`close`, once per call, even on error or panic. It does not prove a real writer's destructor
nonblocking or panic-free: `W::drop` still runs, and a `BufWriter<W>` can itself flush on drop.
Choose and audit the concrete writer when cleanup has a latency or panic requirement.
An aborting profile does not run this unwind recovery.

Sources: [`Write::flush`](https://doc.rust-lang.org/1.100.0/std/io/trait.Write.html#tymethod.flush),
[`Drop` and panics](https://doc.rust-lang.org/1.100.0/std/ops/trait.Drop.html#panics).

The auto-trait proofs, the two reference fabrications, the aliasing models, and the Miri and
clippy invocations are in [miri-and-aliasing.md](miri-and-aliasing.md).

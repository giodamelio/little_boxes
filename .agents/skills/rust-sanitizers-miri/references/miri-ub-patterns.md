# Miri UB Patterns Reference

Worked examples of the undefined behaviour (UB) classes that Miri detects, the
message Miri prints, and the correct pattern. Use this file when a Miri run
fails and you need the shape of the defect, or when you decide which tool runs
on each crate ("Sanitizer comparison" and "Coverage decision table"). The last
section maps ASan error names to their usual Rust causes.

Contents:

- Dangling pointer after reallocation
- Invalid values: transmute and misaligned reads
- Stacked Borrows and Tree Borrows disagree
- `RefCell::as_ptr` fabrication
- `Box` plus FFI aliasing
- Uninitialized memory
- Lifetime extension through a raw pointer
- Sanitizer comparison
- Coverage decision table
- ASan error names

The Miri messages were measured on nightly 2026-11-09 (`miri 0.1.0
(ac8310ba64 2026-11-09)`). Tags such as `<TAG>` and allocation ids such as
`allocN` change with the program, the toolchain, the seed and the flags. Match
on the rest of the text.
Re-measure a message before you quote it for a newer nightly.

The stub strategies are in `references/ffi-stubs.md`.

## Dangling pointer after reallocation

Any operation that can reallocate frees the old buffer. Every pointer derived
before it dangles.

```rust
// Wrong: the pointer is reused after a reallocation.
let mut v: Vec<u32> = Vec::with_capacity(4);
let ptr = v.as_ptr();
v.extend([1, 2, 3, 4]);
v.push(5);                  // Capacity exceeded: the buffer moves.
let val = unsafe { *ptr };  // UB: the pointer is dangling.
// Miri: memory access failed: allocN has been freed, so this pointer is dangling
```

```rust
// Correct: derive the pointer after the last operation that can reallocate.
let mut v: Vec<u32> = Vec::with_capacity(4);
v.extend([1, 2, 3, 4, 5]);
let ptr = v.as_ptr();       // Fresh pointer into the current buffer.
let val = unsafe { *ptr };
```

Same defect class:

- A slice taken from a `Vec` and held across a `push`, `insert`, `reserve`,
  `extend` or `shrink_to_fit`.
- A pointer into a `String` held across a `push_str`.
- A pointer into a `HashMap` value held across an insert.

A pointer moved past the end of its allocation fails earlier, at the
arithmetic: `in-bounds pointer arithmetic failed: attempting to offset pointer
by 20 bytes, but got allocN which is only 12 bytes from the end of the
allocation`.

## Invalid values: transmute and misaligned reads

Producing an invalid value is UB at once. Miri validates every typed copy, so
it reports the `transmute` itself, not a later use of the value.

```rust
// UB: an enum discriminant that no variant uses.
#[repr(u8)]
enum Kind { A = 0, B = 1, C = 2 }

let k = unsafe { std::mem::transmute::<u8, Kind>(99) };
// Miri: constructing invalid value of type Kind: at .<enum-tag>,
//       encountered 0x63, but expected a valid enum tag
```

```rust
// UB: a bool that is not 0 or 1.
let b = unsafe { std::mem::transmute::<u8, bool>(2) };
// Miri: constructing invalid value of type bool: encountered 0x02,
//       but expected a boolean
```

A read through a misaligned raw pointer is UB on every target. It is not a
hardware question: the Rust reference makes any access based on a misaligned
pointer UB.

```rust
// UB: a raw-pointer read of a u32 at an odd offset.
let data = [0u8; 5];
let ptr = data[1..].as_ptr() as *const u32;
let val = unsafe { *ptr };
// Miri: accessing memory based on pointer with alignment 1,
//       but alignment 4 is required
```

Default Miri reports this read on some seeds and passes on others, because the
concrete address of `data` changes with the seed. The result depends on the
seed and on the program layout, and the reported alignment (`1`, `2`) depends
on the address. Match on `accessing memory based on pointer with alignment`.
Add `-Zmiri-symbolic-alignment-check` and the read fails on every seed.

Correct patterns:

- Decode an external tag with `TryFrom` and return an error for an unknown
  value. Never transmute a byte that came from a file, a socket or FFI.
- Read a field out of an unaligned byte buffer with `ptr::read_unaligned`, or
  with `u32::from_le_bytes` on a copied array. The byte-array form needs no
  `unsafe`.

## Stacked Borrows and Tree Borrows disagree

A reborrow of a raw pointer becomes invalid under Stacked Borrows (SB) when the
code accesses the parent pointer. A later use of the reborrow is UB under SB.
Tree Borrows (TB) accepts this program.

```rust
// SB: UB. TB: accepted.
let mut x = 5u32;
let raw = &mut x as *mut u32;
let r = unsafe { &mut *raw };  // Reborrow of the raw pointer.
let v = unsafe { *raw };       // Read through the parent pointer.
*r = 1;                        // Write through the reborrow after that read.
// Miri (SB): attempting a write access using <TAG> at allocN[0x0],
//            but that tag does not exist in the borrow stack for this location
```

The same program without the last write passes under both models. The UB is
the use of `r` after the access through `raw`, not the read itself.

```rust
// Correct: end every use of the reborrow before the raw access.
let mut x = 5u32;
let raw = &mut x as *mut u32;
{
    let r = unsafe { &mut *raw };
    *r = 10;
}                              // `r` is not used after this point.
let v = unsafe { *raw };
```

This example is why the SB run is the gate and TB is extra evidence. A TB pass
does not clear an SB failure. The policy is in `SKILL.md`, section 6.

## `RefCell::as_ptr` fabrication

An `unsafe` deref of `RefCell::as_ptr` produces a reference that the `RefCell`
does not track, because `as_ptr` never touches the dynamic borrow counter. The
`rust-unsafe` skill states the rule and the API fixes. These are the two
messages for an `Rc<RefCell<String>>` whose fabricated `&String` is read after a
`borrow_mut()`:

| Model | Message |
|---|---|
| Stacked Borrows, the default | `trying to retag from <TAG> for SharedReadOnly permission at allocN[OFFSET], but that tag does not exist in the borrow stack for this location` |
| `-Zmiri-tree-borrows` | `reborrow through <TAG> at allocN[OFFSET] is forbidden` |

`OFFSET` is target dependent. On a 64-bit target it is `0x18`, because the
`RefCell` borrow flag takes 8 bytes and the `Rc` header takes 16. On
`i686-unknown-linux-gnu` the same program reports `0xc`.

Miri reports this only when the program interleaves the fabricated reference
with the mutation. Read the reference before the `borrow_mut()` instead of
after, and the same `cargo +nightly miri run` exits 0 with no diagnostic. An
iterator built on `RefCell::as_ptr` therefore passes a Miri suite whose tests
never hold a yielded reference across a `borrow_mut()`. A clean Miri run proves
nothing about this pattern. Treat it as UB on inspection and change the API
shape. Miri is a confirmation here, never the gate.

## `Box` plus FFI aliasing

The rule is in `SKILL.md`, section 8, and the correct patterns are in
`references/ffi-stubs.md`. This is the
Miri test for a registration API: foreign code stores a pointer from a `Box`,
Rust writes through the `Box`, and a later foreign call reads the stored
pointer.

```rust
#[repr(C)]
pub struct State {
    pub field: u32,
}

#[cfg(not(miri))]
unsafe extern "C" {
    fn ffi_register(state: *mut State);
    fn ffi_poll() -> u32;
}

#[cfg(miri)]
std::thread_local! {
    static STORED: std::cell::Cell<*mut State> =
        const { std::cell::Cell::new(std::ptr::null_mut()) };
}

// Miri stand-ins: keep the pointer, and read through it later.
#[cfg(miri)]
unsafe fn ffi_register(state: *mut State) {
    STORED.with(|slot| slot.set(state));
}

#[cfg(miri)]
unsafe fn ffi_poll() -> u32 {
    STORED.with(|slot| unsafe { (*slot.get()).field })
}

#[test]
fn box_write_while_foreign_side_holds_pointer() {
    let mut boxed = Box::new(State { field: 0 });
    let raw: *mut State = &mut *boxed;
    unsafe { ffi_register(raw) };
    boxed.field = 42; // Write through the Box while foreign code holds `raw`.
    assert_eq!(unsafe { ffi_poll() }, 42); // UB: Miri reports this read.
}
```

Both models report the read in `ffi_poll`:

| Model | Message |
|---|---|
| Stacked Borrows | `attempting a read access using <TAG> at allocN[0x0], but that tag does not exist in the borrow stack for this location` |
| `-Zmiri-tree-borrows` | `read access through <TAG> at allocN[0x0] is forbidden` |

Replace both stubs with stand-ins that do not keep the pointer
(`ffi_register` ignores it, `ffi_poll` returns `42`), and both models pass the
same test with no diagnostic. Only a stub pair that keeps the pointer and
dereferences it later exposes the defect. The fixed version moves the value
out with `Box::into_raw`, accesses it only through the raw pointer, and calls
`Box::from_raw` once, after the foreign side unregisters it and no foreign call
or callback can still use the pointer. That version passes under both models.

## Uninitialized memory

```rust
use std::mem::MaybeUninit;

// UB: a read before initialization.
let mut uninit: MaybeUninit<u64> = MaybeUninit::uninit();
let ptr = uninit.as_mut_ptr();
let val = unsafe { ptr.read() };
// Miri: reading memory at allocN[0x0..0x8], but memory is uninitialized
//       at [0x0..0x8], and this operation requires initialized memory
```

```rust
use std::mem::MaybeUninit;

// UB: partial initialization.
let mut buf = MaybeUninit::<[u8; 4]>::uninit();
let p = buf.as_mut_ptr() as *mut u8;
unsafe { p.write(1) };                   // Only byte 0 is initialized.
let arr = unsafe { buf.assume_init() };  // Bytes 1..4 are uninitialized.
// Miri: constructing invalid value of type [u8; 4]: at [1],
//       encountered uninitialized memory, but expected an integer
```

Correct pattern for a buffer that you fill later: allocate with
`vec![0u8; len]`. The zero fill is cheap and safe, and the optimizer removes it
when the whole buffer is overwritten. Use `MaybeUninit` only after a
measurement shows that the fill is a real cost, and write every byte before
`assume_init`.

## Lifetime extension through a raw pointer

```rust
// UB: a reference to a local escapes the function.
fn bad<'a>() -> &'a u32 {
    let x = 42u32;
    unsafe { &*(&x as *const u32) }
}
// Miri, at the caller: constructing invalid value of type &u32:
//       encountered a dangling reference (use-after-free)
```

The borrow checker rejects the safe form of this code. A cast through a raw
pointer defeats the check and produces UB instead of a compile error. Never use
a raw-pointer cast to silence a lifetime error.

The `dangling_pointers_from_locals` lint (warn by default since Rust 1.91)
catches the form that returns `&x as *const u32` directly. It does not fire on
the `&*(&x as *const u32)` form above (checked on Rust 1.99.1).

## Sanitizer comparison

| Tool | Detects | Requires | Overhead |
|---|---|---|---|
| Miri | UB in safe and unsafe Rust, per the language rules | nightly; Rust code only | much slower than native; keep the test set small |
| ASan | Heap and stack memory errors at runtime | nightly for the Rust build | typical 2x slowdown (clang docs) |
| HWASan | Heap and stack memory errors, tag-based | nightly; arm64; Android 14+ with `wrap.sh`, or Android 10 to 13 with a HWASan system image | about 2x CPU, 10% to 35% RAM (NDK docs) |
| MTE | Tagged native heap errors; stack errors with a separate instrumented build | arm64 Android 13+ on a device that reports the `mte` feature | lower overhead in async mode; measure the app |
| TSan | Data races at runtime | nightly for the Rust build | typical 5x to 15x slowdown, 5x to 10x memory (clang docs) |
| MSan | Reads of uninitialized memory | nightly; every object instrumented | typical 3x slowdown (clang docs) |
| UBSan | Integer UB, null dereference and similar, inside C or C++ code | clang or gcc `-fsanitize=undefined` on the C or C++ dependency; rustc has no UBSan option | small runtime cost (clang docs) |
| `loom` | Interleavings of an atomics-based structure, within its model | stable | high; bounded model |

`loom` has model limits. The `rust-test-tools` skill, when it is installed,
owns them; read its loom section before you read a `loom` pass as proof.

## Coverage decision table

Use this table to decide which tool to run against each crate in a workspace.

| Crate shape | Miri | Sanitizers | Notes |
|---|---|---|---|
| `#![forbid(unsafe_code)]`, pure Rust | Yes | Optional | Miri still finds UB reached through dependencies. A `forbid` crate can still reach C through a safe wrapper crate: use the `-sys` row then |
| Hand-written `unsafe` on raw pointers | Yes, required | ASan, and TSan if threaded | Run the SB gate, then TB as extra evidence |
| Parses bytes through raw pointers | Yes, with `-Zmiri-symbolic-alignment-check` | ASan | Add a big-endian run: `--target s390x-unknown-linux-gnu` |
| Uses `MaybeUninit` or partial initialization | Yes, required | MSan | Miri names the uninitialized byte range |
| Concurrency with atomics | Yes, with `-Zmiri-many-seeds` | TSan | Add `loom` for model-checked interleavings |
| Depends on a `-sys` crate that links C or C++ | No | ASan or HWASan, required | Exclude from the Miri run |
| Declares its own `extern "C"` functions | Yes, with `#[cfg(miri)]` stubs | ASan on the real path | The stub removes Miri coverage of the foreign side |
| Generated FFI scaffolding, for example UniFFI or JNI | No | HWASan or MTE on the device | Gate the crossing tests with `#[cfg_attr(miri, ignore)]` |
| Inline assembly | No, unless a fallback exists | ASan on the real path | Add a pure-Rust fallback behind `#[cfg(miri)]` |
| `std::arch` intrinsics | Partly | ASan on the real path | Miri implements a subset. Enable the feature for the run with `RUSTFLAGS="-Ctarget-feature=+avx2"` (and `RUSTDOCFLAGS` for doctests); on a non-x86 host add `--target x86_64-unknown-linux-gnu`. Add a fallback for what Miri rejects |

## ASan error names

| ASan error | Likely Rust cause |
|---|---|
| `heap-buffer-overflow` | `unsafe` slice or pointer access past the end of a buffer |
| `heap-use-after-free` | Raw pointer kept across a `Vec` or `String` reallocation, or used after `drop()` |
| `stack-use-after-return` | Raw pointer to a local that escaped its function |
| `double-free` | `Box::from_raw` called twice on one pointer, or ownership passed to FFI and also dropped in Rust |
| `alloc-dealloc-mismatch` | C++ `new` memory freed with `free`, or the reverse, across the FFI boundary |

The triage order is in `SKILL.md`, section 10.

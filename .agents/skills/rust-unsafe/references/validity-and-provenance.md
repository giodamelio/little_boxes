# Validity and Pointer Provenance

Use this reference before you turn bytes into a typed value, create a reference from a raw
pointer, or perform pointer arithmetic through an integer address.

Contents: invalid typed values, `mem::zeroed`, and `MaybeUninit`; reference creation;
provenance; review procedure; authoritative references.

## Invalid typed values are immediate undefined behavior

Every Rust type has a validity invariant. Creating a value outside that invariant is undefined
behavior at creation time. A later read or dereference is not required.

Common invalid values include:

- a null, dangling, or misaligned reference;
- a `bool` whose byte is not `0` or `1`;
- an enum with no matching discriminant;
- a `NonNull<T>` that contains zero;
- a null function pointer;
- a struct, tuple, or array with any field that is itself invalid.

A broken safety invariant, such as non-UTF-8 bytes in a `str`, is not immediate undefined
behavior. It is still unsound: any safe consumer of the value may then cause undefined behavior.

Do not use `mem::zeroed`, `transmute`, or `MaybeUninit::assume_init` to create such a value and
plan to overwrite it later. Keep the storage as `MaybeUninit<T>` until every byte is initialized
and the complete value is valid.

`mem::zeroed()` is sound only when the documented contract of the exact type proves that
all-zero bytes is a valid value, and the SAFETY comment must cite that contract. `repr(C)` and
an output-only C API do not prove it. Zero is never valid for `NonNull`, `NonZero`, a
reference, `Box`, or a function pointer, and for `bool` or an enum only as `false` or a declared
discriminant. Do not use `zeroed()` for a type with a non-trivial `Drop`: the destructor later
runs on a value that no constructor built. Use `MaybeUninit<T>` when the proof is absent. Call
`assume_init` only after the API reports that it wrote a complete, valid `T`.

For a partially initialized array, track the initialized prefix. On an error, drop only that
prefix, for example with `assume_init_drop` on the initialized sub-slice (stable on
`[MaybeUninit<T>]` since 1.93). Convert to `[T; N]` only after all elements exist.

## Reference creation carries the full contract

Creating `&T` or `&mut T` from a raw pointer asserts more than non-nullness. The pointer must be
aligned, dereferenceable for the reference lifetime, initialized with a valid `T`, and compliant
with the aliasing rules. `&mut T` also asserts exclusive access for its lifetime.

Do not create a reference only to recover a raw pointer or compute a field address. Use a raw
borrow such as `&raw const place`, `&raw mut place`, or `ptr::addr_of!` without an intermediate
reference.

Do not extend the lifetime of a reference returned from raw parts. The caller must prove the
owner, allocation, and foreign access all outlive the reference.

## A pointer is not only an address

A pointer carries provenance that identifies which allocation it can access. Converting the
pointer to an integer address and reconstructing it later can lose that information.

Prefer the Strict Provenance APIs:

| Need | API |
| --- | --- |
| Read the numeric address | `pointer.addr()` |
| Reuse provenance with a new address | `pointer.with_addr(address)` |
| Transform only the address | `pointer.map_addr(transform)` |
| Offset within one allocation | `pointer.add`, `sub`, or `offset` with their exact preconditions |
| A sentinel or tag address that is never dereferenced | `ptr::without_provenance(address)` |
| A non-null, aligned pointer that is never dereferenced | `ptr::dangling()` or `NonNull::dangling()` |

Use `expose_provenance` and `with_exposed_provenance` only when an external interface truly
stores a pointer as an integer and later reconstructs it. This is an explicitly weaker model.
Document the exposure, keep the allocation alive, and test the path under Miri. Do not use an
integer round trip as ordinary pointer arithmetic.

`-Zmiri-strict-provenance` rejects every exposure (Miri nightly-2026-11-09):

```text
error: unsupported operation: integer-to-pointer casts and `ptr::with_exposed_provenance` are not supported with `-Zmiri-strict-provenance`
```

Run the code that exposes provenance in a Miri job without that flag, and keep the strict job
for the rest.

Pointer-to-pointer casts preserve provenance. Pointer-to-integer-to-pointer conversions are the
dangerous boundary. `transmute::<usize, *const T>` creates a pointer with no provenance at all,
and rustc warns on it since 1.91 (`integer_to_ptr_transmutes`); use `without_provenance` or
`with_exposed_provenance` instead.

## Review procedure

1. Name the allocation that supplies provenance.
2. State the valid address range and alignment.
3. State which owner keeps the allocation alive.
4. State when shared or exclusive access starts and ends.
5. Keep storage as `MaybeUninit<T>` until the complete validity proof holds.
6. Run the smallest test under Miri. A path that exposes provenance on purpose runs in a
   separate job without `-Zmiri-strict-provenance`. The `rust-sanitizers-miri` skill, when it is
   installed, owns the Miri flags; run the strict job with
   `MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test --locked`.

Miri is evidence for the executed path. It is not a proof for unexecuted layouts, schedules, or
foreign code.

## Authoritative references

- [Rust Reference behavior considered undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)
- [`MaybeUninit`](https://doc.rust-lang.org/std/mem/union.MaybeUninit.html)
- [`std::ptr` provenance](https://doc.rust-lang.org/std/ptr/index.html#provenance)

# Allocators, zero-sized values, and cells on Rust 1.100

## Keep the allocator with the allocation

The `rust-allocators` skill, when installed, owns allocator selection, contracts,
and FFI transfer. Keep the allocator with `into_raw_parts_with_allocator` or
`into_raw_with_allocator`, and reconstruct with the matching `_in` operation.

## Construct a zero-sized value only with its invariants

Prefer a safe constructor or `Default`. Use `core::mem::conjure_zst::<T>()` only
when `T` is an inhabited zero-sized type and the value preserves its safety
invariants. A zero size does not prove that a type is inhabited. `!` and an empty
enum have no valid value. A zero-sized lock or interpreter token can require
external state even though it has no bytes. `conjure_zst` panics for a nonzero
size; that check does not establish the safety invariants.

## UnsafeCell still needs exclusive access

Rust 1.100 adds `UnsafeCell::replace`, `as_ref_unchecked`, and `as_mut_unchecked`.
They do not permit conflicting references or data races. Prove the required
access for the complete returned reference lifetime. Prefer `get_mut` when an
exclusive borrow of the cell is available. Keep synchronization and reentrancy
proofs in the safety comment.

## Sources

- [Allocator](https://doc.rust-lang.org/1.100.0/std/alloc/trait.Allocator.html)
- [conjure_zst](https://doc.rust-lang.org/1.100.0/core/mem/fn.conjure_zst.html)
- [UnsafeCell](https://doc.rust-lang.org/1.100.0/std/cell/struct.UnsafeCell.html)

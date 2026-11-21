---
description: Use when choosing or implementing a custom allocator, reviewing an Allocator contract, or putting a per-collection allocator into Box or Vec. Covers allocation failure, Layout and alignment, grow and shrink, zero-sized blocks, allocator lifetime and equivalence, and raw ownership across FFI. Not for finding allocation hot spots.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-allocators
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 1e478bc0fab54870a7704be53608a7ad0154ee57
name: rust-allocators
---
# Rust Allocators

Select the allocation boundary before changing the allocator. Rust 1.100 stabilizes
`Allocator` and allocator-aware `Box` and `Vec` operations. It does not stabilize
all allocation APIs or allocator-aware constructors for every collection.
Check the exact method and the package MSRV before use.

## Choose the smallest boundary

| Need | First choice | Evidence before a wider change |
| --- | --- | --- |
| Repeated temporary buffers | Reuse one `Vec` with `clear` and sufficient capacity | Measure allocation calls and retained bytes |
| One collection needs a separate allocation policy | `Vec<T, A>` or `Box<T, A>` | Test the collection's growth, failure, and owner lifetime |
| All allocations in an application need another backend | Application-owned `#[global_allocator]` | Measure the final executable and native dependencies |
| One request or phase owns a region | An allocator backed by a stable region owner | Prove that every collection drops before the region resets |
| Foreign code owns or releases a buffer | An explicit Rust release function and opaque owner | Test one transfer and one release through the real boundary |

Keep a process-wide allocator out of reusable library code. `GlobalAlloc` and
`Allocator` are different contracts; an adapter needs explicit zero-size handling.
The `rust-hot-path` skill, when installed, owns optimization after profiling;
measure allocation calls and bytes with `dhat` before adding allocator parameters to an API.

## Review the unsafe contract

| Operation or condition | Required invariant | Failure to test |
| --- | --- | --- |
| `allocate` | Return a non-null block with at least the requested size and alignment | Refuse a request with `AllocError`; leave other blocks intact |
| Zero-sized allocation | Return a valid aligned non-null result; do not forward a null backend result | Request size zero at a large alignment, then deallocate it |
| `allocate_zeroed` | Initialize all requested bytes to zero | Check every requested byte, not only the first |
| `deallocate` | Accept a live block from an equivalent allocator and a fitting layout | Return each block once after successful and failed resize paths |
| `grow` and `grow_zeroed` | Preserve the old contents; meet the new layout; zero the required extension for `grow_zeroed` | On `Err`, the old pointer remains valid and unchanged |
| `shrink` | Preserve the retained prefix and meet the new layout | A refused shrink also retains the original block |
| Successful resize | The old block is invalid, even when the address is unchanged | Use only the returned pointer and new fitting layout |
| Allocator lifetime | Keep backing storage and allocator identity valid for every live block | Move the owner, end a phase, or reset a region only under a proven lifetime contract |
| Allocation, deallocation, and allocator `Drop` | Do not unwind | Keep formatting, logging, assertions, and panicking instrumentation outside allocator methods |

Use `Layout` to validate size and alignment arithmetic. A fitting layout has the
original alignment and a size within the allocation's documented requested/returned
size range. A pointer to a zero-sized block does not permit a byte access.
Keep live nonzero allocations disjoint. Preserve pointer provenance; an integer
address is not evidence that a new owner can access or free the allocation.

Moving an allocator must preserve its equivalences. A reference to an allocator
is equivalent to the referenced allocator. Follow the allocator's documented
clone/equivalence contract; equal configuration is not proof of shared ownership.
An allocator with inline backing storage needs an immovable owner or another proof
that moving the allocator does not move live blocks. Safe shared methods such as
`reset(&self)` cannot invalidate memory that a collection still owns.

Read [the contract probe](references/contract-probe.md) when implementing an
allocator adapter or adding failure injection. It exercises default resize methods,
zeroing, alignment, zero-size requests, and a failed `Vec::try_reserve_exact`.
Use the same tests for overridden resize methods; passing the adapter probe does
not verify another allocator implementation.

## Handle failure at the caller

Use a fallible collection operation such as `Vec::try_reserve` for input-driven
growth. `Box::new_in` and ordinary `Vec` growth do not turn allocation failure into
an application `Result`. Avoid changing allocator policy solely to make an
infallible constructor appear fallible. Check method stability before suggesting
a `try_*_in` API to a stable-toolchain consumer.

On a failed reserve, check the previous elements and length before retrying or
returning an error. On success, write only initialized values into the collection.
An allocator returns storage, not initialized Rust values or permission to change
`Vec::len` before construction completes.

## Preserve ownership at FFI

Read [raw ownership and FFI](references/ffi-ownership.md) when a buffer leaves Rust,
a callback frees it, or a foreign owner retains it across calls. Preserve the
allocator with `into_raw_parts_with_allocator` or `into_raw_with_allocator` and
reconstruct with the matching `_in` operation.

## Verify the implementation

1. Run `cargo test --locked` for the allocator and its collection clients. Include
   direct allocation failure, failed and successful resize, alignment, zero size,
   zeroing, retained values, and balanced allocation/deallocation paths.
2. Run those tests with `cargo +<compatible-nightly> miri test --locked` when the
   nightly supports the APIs. Record the nightly and target. If it is unavailable,
   retain the host tests and report the missing Miri evidence.
3. Run the target and FFI tests for every shipped boundary. A host allocator test
   does not establish foreign release behavior, thread safety, or region lifetimes.
4. Measure the workload after correctness passes. Report allocation calls, live
   and retained bytes, and latency. Allocation reduction alone is not a speedup.

Source: [Rust 1.100 Allocator contract](https://doc.rust-lang.org/1.100.0/std/alloc/trait.Allocator.html).

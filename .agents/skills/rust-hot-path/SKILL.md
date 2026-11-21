---
description: Use when changing Rust code at a hot spot that a profile or benchmark names, such as cutting allocations (with_capacity, reserve_exact, clone_from, SmallVec, ThinVec, swap_remove), shrinking a type (print-type-sizes, memcpy, large_enum_variant), choosing a hasher (FxHashMap), removing a bounds check, placing inline(always) or a cold path hint, or buffering I/O (BufWriter). Not for choosing a profiler (use rust-performance) or input most calls return unchanged (use rust-copy-on-write). Triggers on "reduce allocations", "too many allocations", "this type is too big", "which hasher".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-hot-path
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: c7c30fdc25ab3fec8405ae0fa24f6adbc4080359
name: rust-hot-path
---
# Rust hot path

What to change in the code after a profile names the hot spot. The `rust-performance` skill
produces the profile and owns benchmark setup; this skill turns the profile into a diff. Apply a
rule here only when a measurement points at it: optimized code is harder to read, so each change
must pay for the readability it spends.

Figures were measured on rustc 1.97.0, aarch64 and x86_64, unless a line names another version.
Std growth policy and layout are unspecified: re-measure before you depend on a figure.

## Route the profile to a section

| The profile shows | Change | Section |
| --- | --- | --- |
| `SipHasher`, `hashbrown` hot | Hasher choice | [Lookups](#lookups) |
| `write`, `read` syscalls dominate | Missing buffering | [I/O](#io) |
| `malloc`, `free`, `__rust_alloc` hot | Allocation rate | [Allocation rate](#allocation-rate) |
| `memcpy` hot with no obvious copy | A type crossed the inline-copy boundary | [type-size-reduction.md](references/type-size-reduction.md#type-size) |
| `memmove` under `Vec::remove` or `Vec::insert` | Each call shifts the tail. Use `swap_remove` when order does not matter, `VecDeque` for a queue, one `retain` for many removals | — |
| `core::panicking::panic_bounds_check` in the disassembly | Bounds checks the compiler could not remove | [inlining-and-codegen.md](references/inlining-and-codegen.md#bounds-checks) |
| A scalar loop where you expected vector code | Bounds checks, or `opt-level` 1, `"s"`, or `"z"` | [inlining-and-codegen.md](references/inlining-and-codegen.md#bounds-checks) |
| Function entry and exit costs, many small calls | Inlining | [inlining-and-codegen.md](references/inlining-and-codegen.md#inlining) |
| Nothing stands out, the work itself is the cost | Algorithm or data structure. Stop here | — |

The last row is the common one: a better algorithm beats every rule below.

## Done when

- The change names a profile, a benchmark delta, or a measured distribution. No number, no merge.
- The metric is stated: allocations per operation, bytes, ns per item, or instructions.
- The measurement came from the shipped profile: non-incremental, at its real `opt-level`, not
  from `dev`.
- A hasher swap states where the keys come from.
- An `unsafe` shortcut such as `get_unchecked` carries a SAFETY comment and a benchmark that
  justifies it.
- A new inline attribute carries a before and after number, because the inliner's budget is
  global and forcing one function in can push a neighbour out. Reject one in review that has none.
- The win has a guard: a `const` size assert, a `dhat` count, or a Criterion baseline.
- Every deliberate special case carries a comment that names its measurement, or the next reader
  simplifies it away. Name the share and the workload, as in
  `// 99% of calls carry 0 or 1 elements (measured 2026-08, ingest benchmark)`.

## Verify and pin the win

Each check proves one claim, and each has a trap that prints a clean result for the wrong reason.

```bash
# A bounds check is gone: the count of checks left in the probe file.
rustc -O --emit asm --crate-type=lib probe.rs -o out.s && grep -c 'panic_bounds_check' out.s

# The layout of a type: discriminant, variants largest first, fields, padding. Nightly only;
# on stable, print size_of, align_of, and core::mem::offset_of! for the suspect types.
# Scoped to one crate; the RUSTFLAGS form dumps every dependency and invalidates the cache.
touch src/lib.rs && cargo +nightly rustc --release -q -- -Zprint-type-sizes

# A dependency function crosses the crate boundary with no attribute: no line names it.
# binutils >= 2.36 demangles v0; with an older nm, pipe through rustfilt before the grep.
cargo build --release -p dep && nm -g --demangle target/release/libdep.rlib 2>/dev/null | grep 'dep::helper'
```

- Mark every probe function `#[inline(never)]`. Without it a small `pub fn` with no caller is
  not emitted, and `grep -c` prints 0 exactly like a removed check.
- Keep the `touch`: a second run of an unchanged crate prints nothing.
- Run the `nm` probe in a non-incremental release build. Incremental and `dev` builds skip the
  inference, so every helper reads as "not inlinable".

An optimization that nothing guards is removed by the next refactor. Pin a type size at compile
time (a mismatch fails with E0080), gated on one architecture because sizes differ per target.

```rust
pub struct Header { id: u64, flags: u32 }

#[cfg(target_arch = "aarch64")]
const _: () = assert!(std::mem::size_of::<Header>() == 16);
```

Use one `dhat` heap test per integration-test binary: two profilers in one process panic.
Keep its `#[global_allocator]` out of library `src/`; it affects every linked binary.
Read [allocation reduction](references/allocation-reduction.md) when writing the test.
Guard time with a Criterion baseline through the `rust-performance` skill.
Read [pointer mapping](references/smart-pointer-map.md) when using `Box::map`,
`Rc::map`, or `Arc::map`; measure reuse and test shared owners, layouts, and panic.

## Lookups

The default hasher is SipHash 1-3. It resists collision flooding and it is slow for short
keys. Replacing it is the largest single win available on a hash-heavy workload, and it
removes a security property.

| Keys come from | Hasher | Why |
| --- | --- | --- |
| Anything a caller outside the process controls: headers, query strings, JSON keys, archive entry names | Keep std `RandomState` | The random per-process seed is the HashDoS defence |
| Untrusted keys, and hashing is measured hot | `ahash::RandomState` | Fast and still randomly seeded per process |
| Keys inside the trust boundary that still want a seed | `foldhash::fast::RandomState` | The default hasher of `hashbrown` 0.16 and 0.17. Seeded without the OS RNG; its docs call it minimally DoS-resistant |
| Internal keys: interned symbols, node indices, enum tags | `rustc_hash::FxHashMap` | Fastest measured, unseeded |
| Counters and dense integer ids | `nohash_hasher::IntMap` | Identity hash. Wrong for ids with constant low bits |

`FxHasher` was 2.2x to 5.0x faster than SipHash on 1M inserts. Do not pick `fnv`.

`FxHashMap` is a type alias for `HashMap` with a different hasher, and std supplies `new` and
`with_capacity` only for `RandomState`, so both fail with E0599. Write `FxHashMap::default()`
or `HashMap::with_capacity_and_hasher(n, FxBuildHasher)`.

Enforce one choice across a workspace with `disallowed-types` in `clippy.toml`. Banning
`std::collections::HashMap` by path does not flag `FxHashMap`, even though the alias is
that type: clippy matches the written path.

Read [references/hashing-and-io.md](references/hashing-and-io.md) when you need the full hasher
measurements, the two-process seeding check, the collision failure mode, the `nohash-hasher`
key rules, or the byte-wise `ByteHash` derives.

## I/O

**File writes are unbuffered.** A `writeln!` to a `File` costs at least one `write` syscall.
**Locking stdout does not help on its own:** `Stdout` is a `LineWriter`, one syscall per
newline, lock or no lock. Wrap the file or the lock in a `BufWriter` (`BufWriter::new(stdout().lock())`).

Always end a `BufWriter` with an explicit `flush()?`, because a dropped one loses its final
write error and the file ends truncated with nothing in the log. `into_inner()` is not a
substitute: it drains the buffer without flushing the inner writer. The default buffer is 8 KiB;
use `with_capacity` when one logical record is larger.

**`BufRead::lines` allocates one `String` per line.** Read with `read_line` into one `String`
that you `clear` each iteration. `read_line` keeps the line terminator. Remove it with
`strip_suffix`, never with `trim_end()`, which also deletes the trailing spaces and tabs that
`lines()` keeps. Read [references/hashing-and-io.md](references/hashing-and-io.md) when you
rewrite a writer or a reader: it holds the measurements, the `strip_eol` helper, and the
`read_until` trade.

## Allocation rate

**`Vec` does not double from 4.** The first non-zero capacity depends on the element size (8,
4, or 1), and each growth then doubles: twenty `push` calls on a `Vec<u32>` cost four
allocations. One `Vec::with_capacity(20)` costs one. **`reserve` applies the amortized policy on
top of your request; `reserve_exact` does not.** Use `reserve_exact` when you know the final
length and you are memory-bound. Use `try_reserve` when the size comes from untrusted input; the
`rust-panic-safety` skill covers it.

**`clone_from` reuses the destination buffer.** `a = b.clone()` allocates every time;
`a.clone_from(&b)` writes into the buffer `a` already holds. The two look identical in review, so
enable `clippy::assigning_clones` (`pedantic`, off under a plain `cargo clippy`). `Vec::clone()`
also drops the reserve, so a clone never hands a pre-warmed buffer to another owner.

Read [references/allocation-reduction.md](references/allocation-reduction.md) for the growth ladder, the `reserve` and `clone_from`
probes, and the measured counts.

**Keep one workhorse buffer.** Declare the buffer outside the loop and `clear` it at the top of
each iteration. `clear` keeps the allocation; assigning a fresh collection throws it away. Do
not call `shrink_to_fit` in such a loop: it reallocates every time, which turns a
zero-allocation loop back into one allocation per iteration. It is a footprint tool, never a
speed tool.

**Make `collect` exact.** A hand-written iterator's default `size_hint` is `(0, None)`, and an
inexact adaptor such as `filter` loses the length, so `collect` runs the growth ladder (13
allocations for 10,000 items, against 1). When you know the output length, use `with_capacity`
plus `extend`. The `rust-iterator-impl` skill holds the `ExactSizeIterator` contract.

**Format into one reused buffer.** `format!` allocates a `String` per call. `write!` into one
cleared buffer, or, for one integer, use `format_into` (Rust 1.98).

Read [references/allocation-reduction.md](references/allocation-reduction.md) when the
allocation is a `HashMap` reserve, a `collect` chain, a zero-filled buffer, an inline-capacity
type such as `SmallVec` or `ArrayVec`, an `Rc`/`Arc` refactor, a formatting loop, or an eager
`ok_or` argument.

## Type size

Read [type-size-reduction.md](references/type-size-reduction.md#type-size) when `memcpy` is hot
with no obvious copy, or a type is instantiated often: it holds the target copy boundaries, the
cache-line threshold, boxing the outsized variant, `ThinVec` and `Box<[T]>`, and the `repr` rule.

## Bounds checks

Read [inlining-and-codegen.md](references/inlining-and-codegen.md#bounds-checks) when the
disassembly shows `panic_bounds_check` or a loop you expected to vectorize stays scalar: it holds
the three safe shapes, `as_chunks`, the `opt-level` rule, and the bar for `get_unchecked`.

## Inlining

Read [inlining-and-codegen.md](references/inlining-and-codegen.md#inlining) when you add, remove,
or review `#[inline]`, `#[inline(always)]`, `#[inline(never)]`, `#[cold]`, or
`core::hint::cold_path()`.

The `rust-lints` skill, when it is installed, holds these rules as lints and the `clippy.toml`
thresholds; the `rust-discipline` skill holds signature shape (`&str` against `&String`).

# Data-shape traps

Read this file when a diff adds a `HashMap` or `HashSet` key type, an `Ord` or `PartialOrd` impl,
reverses or truncates text, or builds a fixed-size array for heap residence. Each trap is silent
on an ASCII fixture or a small value, and appears with real data, on a constrained stack, or after
a toolchain upgrade.

Each item states a severity, a wrong example, a correct example, and a rule.

Contents:

- `Hash` and `PartialEq` contract violation
- `PartialOrd` and `Ord` disagree
- `chars().rev()` corrupts combining marks and ZWJ sequences
- Large stack arrays and the `Box::new([0u8; N])` pitfall

---

## `Hash` and `PartialEq` contract violation

**Severity: CRITICAL**

The standard library requires that `k1 == k2` implies `hash(k1) == hash(k2)`. The contract breaks
when `eq` treats two values as equal that `hash` feeds differently. `HashMap` and `HashSet` then
return silently incorrect results. Two shapes do this: a manual `PartialEq` next to a derived
`Hash`, and two manual impls that normalize differently. A derived `PartialEq` next to a manual
`Hash` over the same fields is safe.

```rust
// BUG: the derived Hash uses the original case; the manual PartialEq ignores case.
#[derive(Hash)]
struct Tag(String);
impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_lowercase() == other.0.to_lowercase()
    }
}
impl Eq for Tag {}
// HashSet<Tag> stores "Foo" and "foo" as two different entries.
```

Clippy's `derived_hash_with_manual_eq` (correctness, deny by default) rejects the first shape
with `you are deriving 'Hash' but have implemented 'PartialEq' explicitly`. No lint sees the
second. Measured on clippy 1.98.1.

Rule: when you write a custom `PartialEq`, write a matching custom `Hash` that hashes the same
normalized form. Add a test that inserts through one form and looks up through the other.

---

## `PartialOrd` and `Ord` disagree

**Severity: WARNING**

`PartialOrd` and `Ord` must agree. Since Rust 1.98, `derive(PartialOrd)` next to `derive(Ord)` on
a type with no generic or lifetime parameters calls `Ord::cmp` (rust-lang/rust#155598). A field
whose manual `PartialOrd` and `Ord` disagree therefore changes `<`, `>`, and `sort()` results on
the toolchain upgrade, with no diagnostic from rustc. A generic type keeps the old derived code.

```rust,run
use std::cmp::Ordering;

// BUG: `cmp` sorts descending, `partial_cmp` sorts ascending.
#[derive(PartialEq, Eq)]
struct Rank(u8);
impl Ord for Rank {
    fn cmp(&self, other: &Self) -> Ordering { other.0.cmp(&self.0) }
}
impl PartialOrd for Rank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.0.cmp(&other.0)) }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Entry(Rank);

fn main() {
    assert!(Rank(1) < Rank(2));
    // Rust 1.97 gives `true` here; 1.98 routes the derived `<` through `Rank::cmp`.
    assert!(!(Entry(Rank(1)) < Entry(Rank(2))));
}
```

Rule: make every manual `PartialOrd` on an `Ord` type return `Some(self.cmp(other))`. Clippy's
`non_canonical_partial_ord_impl` (suspicious, warn by default) flags any other body. Measured on
clippy 1.98.1. It cannot see a disagreement inside a dependency's type.

---

## `chars().rev()` corrupts combining marks and ZWJ sequences

**Severity: CRITICAL**

A `char` is a Unicode scalar value, not a user-visible character. Reversing scalars moves a
combining mark onto the character that preceded it, and reverses the order of a ZWJ emoji
sequence. The result is still valid UTF-8, so nothing errors and no lint fires.

Measured on rustc 1.97.0, edition 2024, with `s.chars().rev().collect::<String>()`:

| Input | Output | Effect |
| --- | --- | --- |
| `"noe\u{301}l"` | `"l\u{301}eon"` | the acute accent moved from `e` to `l` |
| `"\u{1F469}\u{200d}\u{1F680}"` | `"\u{1F680}\u{200d}\u{1F469}"` | the ZWJ order reversed |
| `"abc"` | `"cba"` | correct, which is why an ASCII fixture passes |

```rust
// BAD: reverses scalar values, not characters. "noe\u{301}l" becomes
// "l\u{301}eon" — U+006C U+0301 U+0065 U+006F U+006E — which renders as "ĺeon".
fn reverse(text: &str) -> String {
    text.chars().rev().collect()
}

// GOOD: reverse grapheme clusters, from the `unicode-segmentation` crate.
// text.graphemes(true).rev().collect::<String>()
```

Rule: std has no correct reverse for arbitrary text. Use `graphemes(true)` from the
`unicode-segmentation` crate. The rule covers only the operations where order or user-visible
character boundaries matter: reverse, truncate, centre, pad, and per-character iteration for
display. `chars().count()`, `chars().filter()`, and byte-oriented parsing of a known ASCII grammar
do not reorder text and are not affected.

---

## Large stack arrays and the `Box::new([0u8; N])` pitfall

**Severity: WARNING**

`Box::new([0u8; N])` does not allocate `N` bytes directly on the heap. The expression first
builds `[0u8; N]` on the caller's stack, then `Box::new` copies it into a heap allocation. A
debug build performs no placement optimization, so it materializes one full stack copy. It
overflows once `N` approaches the thread's stack size, and mobile and embedded targets commonly
give a thread about 1 MiB to 2 MiB. Measured on rustc 1.98.1 debug, aarch64-apple-darwin, in a
1 MiB thread: `N = 1000 KiB` passes, `N = 2 MiB` overflows. A release build sometimes removes
the copy through NRVO, but that optimization is fragile. Any intermediate
`let buf = Box::new([0u8; N]);` can materialize the stack copy again.

```rust
// BAD: builds the array on the stack first, then copies it to the heap.
let on_stack: Box<[u8; 1024 * 1024]> = Box::new([0u8; 1024 * 1024]);

// BAD: returning a large array by value forces a memcpy through the stack.
fn make_buf() -> [u8; 1024 * 1024] { [0u8; 1024 * 1024] }

// GOOD: allocate on the heap from the start. `vec!` allocates zeroed memory
// directly, and the capacity equals the length, so nothing is copied.
let heap: Box<[u8]> = vec![0u8; 1024 * 1024].into_boxed_slice();

// GOOD: the same allocation, typed as a fixed-size array.
let heap_array: Box<[u8; 1024 * 1024]> = vec![0u8; 1024 * 1024]
    .into_boxed_slice()
    .try_into()
    .expect("the length is exactly 1 MiB");
```

Rule: build any array that a stack-size lint flags for heap residence with
`vec![x; n].into_boxed_slice()`, or collect an exact-length iterator into `Box<[T]>`. Use
`Box::new_uninit_slice` only when you write every element yourself. Never write `Box::new([T; N])` for a large `N`, and never return
`[T; N]` by value for a large `N`.

Two lints are the mechanical check, and the `rust-lints` skill owns both thresholds in
`clippy.toml`. `clippy::large_stack_arrays` (pedantic) flags one array over
`array-size-threshold`, default 16384 bytes: measured on clippy 1.98.1, it flags
`Box::new([0u8; 20000])` with `allocating a local array larger than 16384 bytes`.
`clippy::large_stack_frames` flags a whole frame over `stack-size-threshold`, which the
`rust-lints` template lowers to 4096 bytes. That limit is stricter because a frame sums every
local: measured on clippy 1.99.1, an 8000-byte local array gives `this function may allocate
8026 bytes on the stack`, and `large_stack_arrays` stays silent.

`Vec::into_boxed_slice` is not free when capacity exceeds length. The `rust-hot-path` skill, when
it is installed, owns the measured table; the call runs `shrink_to_fit`, a `realloc` that can
move the buffer. When the
length is exact, build the boxed slice straight from the iterator, which allocates once:

```rust
let squares: Box<[u32]> = (0..1024u32).map(|n| n * n).collect();
```

Find candidates:

```bash
rg "Box::new\(\s*\[0?[a-z0-9_]+\s*;\s*[0-9]{4,}" --type rust -n
rg "fn .* -> \[[a-z0-9_]+\s*;\s*[0-9]{4,}\]" --type rust -n
```

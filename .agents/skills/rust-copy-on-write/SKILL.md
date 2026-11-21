---
description: Use when choosing between borrowed and owned data at an API boundary (Cow<str>, Cow<[T]>, to_mut, into_owned), including a function that a profile shows allocating a new String or Vec on every call although most inputs come back unchanged, or when clone cost drives a data-structure choice (Arc<str>, the persistent collection crates im, imbl, rpds). Not for other allocation cuts at a named hot spot (use rust-hot-path). Triggers on "copy-on-write", "borrowed or owned", "clone cost", "structural sharing".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-copy-on-write
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 1ebdb1b45247e71898277843e2b06d49f863aedc
name: rust-copy-on-write
---
# Rust copy-on-write

Every number below was measured on rustc 1.98.1, aarch64-apple-darwin, release profile.
Allocation counts repeat exactly across runs; times do not. Re-measure on your target.

## Route the decision to a section

| You are about to | Go to |
| --- | --- |
| Return `String` where most calls change nothing | [`Cow` in return position](#cow-in-return-position) |
| Chain two or more string or slice transforms | [`Cow` in argument position](#cow-in-argument-position) |
| Write `cow.to_mut()` in front of a method | [`to_mut()`](#to_mut-is-for-mut-self-methods-only) |
| Put a `Cow` field in a struct, or share a value a cache or a task holds | [A `Cow` field infects the struct](#a-cow-field-infects-the-struct-with-a-lifetime) |
| Write `Cow<'_, String>` or `Cow<'_, Vec<T>>` | [The `Cow` parameter is the borrowed half](#the-cow-parameter-is-the-borrowed-half) |
| Write `impl Borrow<..>`, or debug a `HashMap::get` that misses a present key | [references/borrow-and-toowned.md](references/borrow-and-toowned.md) |
| Write `fn with(&self) -> Self` for a version history | [`Cow` is not structural sharing](#cow-is-not-structural-sharing) |
| Choose between `Vec` and a persistent collection | [Persistent collections](#persistent-collections) |
| Read a profile that already names `__rust_alloc` | The `rust-hot-path` skill, when it is installed: capacity, buffer reuse, `SmallVec` |

## Verify the decision

| Claim | Check | A pass does not prove |
| --- | --- | --- |
| The `Cow` shape saves allocations | The counting allocator below, on real input, with the `Borrowed` path in the run | That callers keep the borrow. Read every call site for `.into_owned()` and `.to_string()` |
| No `Cow` names an owned type | `cargo clippy` (`clippy::owned_cow`) plus the grep in [that section](#the-cow-parameter-is-the-borrowed-half) | Anything about custom `ToOwned` types. Anything about public items, unless `clippy.toml` sets `avoid-breaking-exported-api = false`; the grep covers them |
| No `to_mut()` runs before a `&self` method | The grep in [that section](#to_mut-is-for-mut-self-methods-only), then a read of each hit | That each `to_mut()` sits inside the branch that writes |
| A `Borrow<X>` key finds its entries | The `hash_one` test in [references/borrow-and-toowned.md](references/borrow-and-toowned.md), then a `get` through the borrowed form | A correct contract, from a `get` test alone: it passes about 1 run in 150 with a broken contract |
| The collection crate's resolved tree is clean | `cargo deny --config deny.toml --locked check advisories` with `unsound = "all"` in `deny.toml`, or `cargo audit --deny warnings`, on the lockfile | A clean tree, if you drop the setting: the defaults miss a transitive unsound advisory ([references/persistent-collections.md](references/persistent-collections.md) has the case). A failure does not prove that your code reaches the flagged API |

## Measure before you decide

Every rule here is a hit-rate rule, and a hit rate belongs to your workload. Run your real input
through both shapes in a release bench binary with a counting global allocator
([references/counting-allocator.md](references/counting-allocator.md) has one), and compare the
counts. `dhat` gives the same counts (the `rust-performance` skill, when it is installed).

## `Cow` in return position

`Cow<'_, str>` pays only on the branch that changes the data. Over 1000 paths with a 5% rewrite
rate it makes 50 allocations, against 1000 for `String`; a caller that writes `.into_owned()`
puts all 1000 back. [references/cow-signatures.md](references/cow-signatures.md)
has the measurement and a runnable example.

The rule: `Cow` in return position wins **only when the caller can hold the borrow**. A call
site that writes `.into_owned()` or `.to_string()` returns the count to the `String` figure, and
the `Cow` is a branch plus a lifetime parameter for nothing.

`into_owned()` costs 0 allocations on a `Cow::Owned` and 1 on a `Cow::Borrowed`. Call it once
at the boundary where the source buffer dies, not inside the loop that produced the `Cow`.

Two checks before you convert a signature:

- **Measure the branch split.** `Cow` allocates (rate x N) times, `String` always N.
- **Read the call sites.** One `.into_owned()` in the only caller cancels the whole change.

## `Cow` in argument position

A `Cow -> Cow` chain is a large win, because each stage passes its input through untouched
instead of re-allocating between stages. Three chained stages over the same input cost 50
allocations, against 3000 for `fn(&str) -> String` stages
([references/cow-signatures.md](references/cow-signatures.md) has the chain).

Take `Cow<'a, str>` by value in a chain stage. A stage that takes `&Cow<'a, str>` must clone
an `Owned` input only to pass it through.

## `to_mut()` is for `&mut self` methods only

`to_mut()` on a `Cow::Borrowed` clones the data, so `cow.to_mut().replace(..)` allocates twice;
`cow.replace(..)` reaches `str::replace` through `Deref` and allocates once. `to_mut()` also
flips the value to `Owned` even when nothing is written, so every later consumer loses the
zero-copy property. [references/cow-signatures.md](references/cow-signatures.md) has both forms as runnable
probes and the `push_str` cost.

Use `to_mut()` only for a method that takes `&mut self`: `push_str`, `Vec::push`, `sort`. Keep
the call inside the branch that writes, not above it.

`to_mut()` on an already-`Owned` value is free, so a benchmark that starts from `Cow::Owned`
shows no difference and hides both defects. Benchmark the `Borrowed` path.

Grep for the misuse. Every hit needs a check that the method takes `&mut self`:

```bash
rg '\.to_mut\(\)\.' --type rust -n
grep -rnE --include='*.rs' '\.to_mut\(\)\.' .   # without ripgrep
```

## A `Cow` field infects the struct with a lifetime

`struct Header<'a> { name: Cow<'a, str> }` is a borrowing struct, and the lifetime spreads to
every signature that mentions it. The `rust-discipline` skill, when it is installed, covers
lifetime infection. Without it, give a long-lived struct an owned field (`String`) or a generic
parameter, so it carries no lifetime.

Two failures arrive together: E0515 when a function returns a borrow of its local, and E0521
when a borrowed value goes into a `'static` container
([references/cow-signatures.md](references/cow-signatures.md) has the `compile_fail` example).

E0521 stops the design: a cache, a `Vec` that outlives the parse buffer, or a value sent to
another task needs `Header<'static>`. The only exit is a hand-written `into_static(self) ->
Header<'static>` that calls `into_owned()` on each field where the parse buffer dies
([references/borrow-and-toowned.md](references/borrow-and-toowned.md#into_static-the-exit-from-a-borrowing-struct)
has a tested example).

Add a `Cow<'a, str>` field only when a real zero-copy parse path exists and the value dies
inside the borrow scope. A field that is mostly a literal takes `Cow<'static, str>`: it adds no
lifetime parameter and is `Send + 'static`. Otherwise use the "When to use none of this" table.
The `rust-hot-path` skill, when it is installed, has the full `Arc::make_mut` table and the
`Weak` detach trap; [references/cow-signatures.md](references/cow-signatures.md) has the
unsized-newtype case.

## The `Cow` parameter is the borrowed half

`Cow<'a, B>` requires `B: ToOwned`, and `alloc` ships `impl<T: Clone> ToOwned for T`, so
`Cow<'_, String>` type-checks. It is legal and useless: every caller that starts from a `&str`
must allocate a `String` first ([references/cow-signatures.md](references/cow-signatures.md) has
the E0308 detail).

Write `Cow<'_, str>`, `Cow<'_, [T]>`, `Cow<'_, Path>`. Do not write `Cow<'_, String>`,
`Cow<'_, Vec<T>>`, or `Cow<'_, PathBuf>`.

`clippy::owned_cow` (warn by default) names the borrowed half for `String`, `Vec<_>`, `CString`,
`OsString`, and `PathBuf`. It skips public items unless `clippy.toml` sets
`avoid-breaking-exported-api = false`, so grep the public API:

```bash
rg 'Cow<[^>]*(String|Vec<|PathBuf|OsString|CString)' --type rust -n
grep -rnE --include='*.rs' 'Cow<[^>]*(String|Vec<|PathBuf|OsString|CString)' .   # without ripgrep
```

A key whose `Hash` or `Eq` disagrees with its `Borrow` target makes `HashMap::get` silently miss
a present key. Read [references/borrow-and-toowned.md](references/borrow-and-toowned.md)
when you write `impl Borrow<..>` or `impl ToOwned`, when a `HashMap::get` misses a key the map
holds, or when E0119 or E0283 appears around `ToOwned`, `borrow()`, `eq`, or `hash`.

## `Cow` is not structural sharing

`Cow` shares one value between one borrower and one owner. It never shares interior nodes. A
`fn with(&self) -> Self` built on `Cow` clones the `Vec` and every element on each call, so a
chain of calls is exactly quadratic. Do not build a version history or an undo stack on `Cow`.
Benchmark with the production element type: a `Vec<u32>` toy passes, and the production
`Vec<String>` workload does not. Read
[references/persistent-collections.md](references/persistent-collections.md) for the `Log::with`
example and the measurement.

## Persistent collections

A persistent collection makes `clone()` free by sharing trie nodes, and charges for it on every
read: at N = 100 000 `i32`, an indexed read pass costs 45x to 89x a `Vec` pass. Take one only
when the clone-to-mutation ratio is high and reads iterate rather than index. Otherwise keep
`Vec` and clone it.

Depend on `imbl = "7.0.2"` or later, not on `im`: `im` and older `imbl` trees resolve crates
with unpatched soundness and double-free advisories.

Read [references/persistent-collections.md](references/persistent-collections.md) before you
add `im`, `imbl`, or `rpds`, and when you review a change that uses one. It holds the cost tables,
the `rpds::Vector::new()` `!Send` trap, the advisory check, and the review checklist.

## When to use none of this

| Situation | Use instead |
| --- | --- |
| The value never changes after construction, and many owners read it | `Arc<str>`, `Arc<[T]>`. Clone is 0 allocations, 16 bytes on the stack |
| One owner mutates, others may hold a stale clone | `Arc::make_mut`, also on `Arc<str>` and `Arc<[T]>` when the length stays fixed |
| The modification rate is above 50% | Plain `String` or `Vec<T>`. The `Cow` saves under half the allocations, and costs a branch and a lifetime parameter |
| The value is indexed in a loop | `Vec<T>`, cloned. 45x to 89x cheaper per read |
| The struct must be `'static` and cross a task boundary | Owned fields, or `Cow<'static, str>` when most values are literals. A `Cow<'a, str>` that borrows a parse buffer cannot be `'static` |

## Review checklist

The [verify table](#verify-the-decision) covers the branch split, the call sites, `to_mut()`,
owned `Cow` parameters, `Borrow` keys, and the advisory gate. Also reject a new `Cow` field on a
struct that a cache or a task must hold (store owned data, add `into_static`, or share through
`Arc`), and a `fn with(&self) -> Self` that clones a `Cow` of a non-`Copy` element type. For a
persistent collection, run the checklist in
[references/persistent-collections.md](references/persistent-collections.md#review-checklist).

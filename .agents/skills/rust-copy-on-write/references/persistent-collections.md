# Persistent collections against `Vec`

Deep material for `rust-copy-on-write`. Read it when clone cost drives the choice of a
collection, or before you add `im`, `imbl`, or `rpds` to `Cargo.toml`.

Contents:

- [A `Cow` version history is quadratic](#a-cow-version-history-is-quadratic)
- [The cost table](#the-cost-table)
- [The API shape decides the write cost](#the-api-shape-decides-the-write-cost)
- [`rpds::Vector::new()` is not `Send`](#rpdsvectornew-is-not-send)
- [Check the resolved dependency tree](#check-the-resolved-dependency-tree)
- [Review checklist](#review-checklist)

A persistent collection makes `clone()` free by sharing trie nodes. It charges for that on
every read.

## A `Cow` version history is quadratic

This example looks persistent and is not:

```rust,run
use std::borrow::Cow;

#[derive(Clone)]
struct Log<'a> {
    lines: Cow<'a, [String]>,
}

impl Log<'_> {
    // Looks persistent. Is not: cloning a Cow::Owned deep-clones every String.
    fn with(&self, line: &str) -> Self {
        let mut next = self.clone();
        next.lines.to_mut().push(line.to_string());
        next
    }
}

fn main() {
    let base = Log { lines: Cow::Owned(vec![String::from("a")]) };
    let next = base.with("b");
    assert_eq!(base.lines.len(), 1);
    assert_eq!(next.lines.len(), 2);
}
```

Measured at 2000 chained `with`
calls that start from `Log { lines: Cow::Owned(Vec::new()) }`, against a plain
`Vec::push` loop over the same 2000 eight-byte `String` values. Times are the best and the
worst of nine passes:

| Loop | Allocations | Bytes | Time |
| --- | --- | --- | --- |
| `with` on `Cow<'_, [String]>` x2000 | 2 004 999 | 159 936 144 | 25-31 ms |
| `Vec::push` x2000 | 2 010 | 114 208 | 16-33 us |

2 004 999 is 2000 * 1999 / 2 = 1 999 000 `String` clones, plus the 2000 new values, plus
3 999 `Vec` allocations. The `Vec` term is one clone and one growth realloc per iteration,
less the clone of the empty base, which allocates nothing. The cost is exactly quadratic.

The false negative that hides this: for `T: Copy` the clone is one `memcpy`. Cloning a
`Cow<'_, [u32]>` of 1000 elements costs 1 allocation and 4000 bytes. The same clone on a
`Cow<'_, [String]>` of 1000 elements costs 1001 allocations and 31 890 bytes. A `Vec<u32>`
toy benchmark passes and the production `Vec<String>` workload does not.

## The cost table

Measured at N = 100 000 `i32` on rustc 1.98.1, edition 2024, aarch64-apple-darwin, release
profile, with the allocator in [counting-allocator.md](counting-allocator.md). Build is a `push_back` loop from an empty
collection. The index pass reads `v[i]` for every `i`; the iterate pass sums `v.iter()`. Each
time cell is the best of nine passes, as the range over five runs. Allocation counts repeat
exactly across runs. Times do not.

| Operation | `Vec` | `im` 15.1.0 | `imbl` 7.0.2 | `rpds` 1.2.1 |
| --- | --- | --- | --- | --- |
| Build, allocations | 16 | 1 594 | 1 619 | 932 349 |
| Build, time | 33-34 us | 0.46-0.47 ms | 0.53-0.58 ms | 19-30 ms |
| Clone, allocations | 1 | 0 | 0 | 0 |
| Clone, time | 5.0-5.1 us | under 42 ns | under 42 ns | under 42 ns |
| Index `[i]`, whole pass | 6.5-6.6 us | 555-564 us | 578-580 us | 299-301 us |
| Iterate, whole pass | 6.6-6.7 us | 172-174 us | 147-152 us | 295-296 us |

42 ns is the timer resolution on that host. Three ratios decide the choice:

- **Clone costs nothing.** 0 allocations against 1, and no measurable time against 5 us for
  the `Vec`.
- **Indexed reads cost 45x to 89x.** `rpds` indexes at 45-46x `Vec`, `im` at 84-87x, and `imbl`
  at 87-89x. `rpds` is the cheapest indexer of the three, and its `&self -> Self` write path
  is the most expensive (next section).
- **Iteration costs 22x to 45x.** `im` and `imbl` iterate three to four times faster than they
  index. `rpds` iterates no faster than it indexes. Iteration is the only read shape that `im`
  and `imbl` serve well.

Take a persistent collection only when both hold: the clone-to-mutation ratio is high, and the
collection is read by iteration rather than by index. Otherwise keep `Vec` and clone it. A `Vec`
clone is one allocation and one `memcpy`, and 5 us for 100 000 `i32` is cheaper than the read
penalty on almost any workload.

## Write cost: the API shape and the crate

Building 100 000 `i32` elements one push at a time (rpds 1.2.1, imbl 7.0.2, Rust 1.99.1, counting
allocator):

| Crate and call | Signature | Allocations |
| --- | --- | ---: |
| `rpds` `push_back` | `&self -> Self` | 932 349 |
| `rpds` `push_back_mut` | `&mut self` | 116 134 |
| `imbl` `push_back` | `&mut self` | 1 619 |
| `im` `push_back` | `&mut self` | 1 594 |

The gap has two parts. The `&self -> Self` shape builds a fresh trie path on every write, kept
version or not, and costs `rpds` a factor of 8 over its own `push_back_mut`. The remaining factor
of about 72 is the `rpds` design itself: its `&mut self` path still allocates far more than the
RRB tree of `im` and `imbl`, whose writes stay in place until the next `clone()`. When only the
newest version survives, call the `_mut` methods of `rpds` (`push_back_mut`, `set_mut`,
`drop_last_mut`), and do not expect them to reach `imbl`.

Do not benchmark one crate and generalise the result to the other.

```rust
// Cargo.toml: imbl = "7.0.2"
fn main() {
    // push_back takes &mut self: it mutates in place until someone holds a clone.
    let mut base: imbl::Vector<i32> = imbl::Vector::new();
    for i in 0..1000 {
        base.push_back(i);
    }

    // clone() is where sharing starts. 0 allocations.
    let mut fork = base.clone();
    fork.push_back(1000);   // copies only the nodes on the path it touches

    assert_eq!(base.len(), 1000);
    assert_eq!(fork.len(), 1001);
}
```

## `rpds::Vector::new()` is not `Send`

`imbl::Vector<T>` is `Arc`-backed by default, so it is `Send + Sync`. `rpds::Vector<T>` is not:
it defaults to the parameter `P = RcK`, an `Rc`-backed archetype. The failure
appears only when the value first crosses a thread, and the error names a dependency's
internals rather than your type:

```text
error[E0277]: `*const ()` cannot be sent between threads safely
   = help: within `archery::shared_pointer::kind::rc::RcK`, the trait `Send`
           is not implemented for `*const ()`
note: required because it appears within the type `rpds::Vector<i32>`
```

Use `rpds::Vector::new_sync()` for anything that reaches a thread pool or an async runtime.
`rpds::VectorSync<T>` is a type alias for the `Arc`-backed form, so `VectorSync::<T>::new()`
does not exist and gives `error[E0599]: no associated function or constant named `new``.
The constructor is `Vector::new_sync()`.

```rust
// Cargo.toml: rpds = "1.2"
fn main() {
    // Vector::new() gives the Rc archetype: fast, single-thread only.
    let local: rpds::Vector<i32> = rpds::Vector::new().push_back(1);

    // Vector::new_sync() gives the Arc archetype: the only Send + Sync one.
    let shared: rpds::VectorSync<i32> = rpds::Vector::new_sync().push_back(1);
    let fork = shared.push_back(2);   // Theta(log n): reuses nodes outside the changed path
    std::thread::spawn(move || assert_eq!(fork.len(), 2)).join().expect("thread joins");

    assert_eq!(local.len(), 1);
    assert_eq!(shared.len(), 1);      // the fork did not touch it
}
```

`rpds::Vector::clone` is O(1). `push_back` is O(log n), because it creates the
changed trie path and shares the untouched nodes. Do not describe the new
version as sharing every node with the old one.

Pin the property with `fn assert_send_sync<T: Send + Sync>() {}` called on your alias in a
test. The bound then fails at build time, not at the first `tokio::spawn`.

## Check the resolved dependency tree

Depend on `imbl = "7.0.2"` or later. Do not add `im`:

- `im` 15.1.0, from 2022, is the last release. Its repository is archived, and `OrdSet`
  insertion violates the aliasing rules from safe code. Both have advisories, and neither
  advisory has a patched version. Its dependencies `sized-chunks` and `bitmaps` are
  archived too, and `sized-chunks` carries its own unpatched soundness advisory.
- `imbl` is the maintained fork, and a maintained direct dependency still does not prove a clean
  tree. `imbl` 7.0.1 resolved `imbl-sized-chunks` 0.1 and `bitmaps` 3. `imbl` 7.0.2 requires
  `imbl-sized-chunks` 0.2 and drops `bitmaps`. One example of what a 7.0.1 lockfile matches:

| Advisory | Crate | Kind | Fixed in |
| --- | --- | --- | --- |
| RUSTSEC-2026-0292 | `imbl-sized-chunks` < 0.2.0 | memory corruption: double free or use-after-free when an element's `Drop` panics | `imbl-sized-chunks` 0.2.0, which `imbl` 7.0.2 requires |

Do not copy advisory lists into code comments or docs; they go stale. Run the gate on the
lockfile. Every `im`-tree advisory above is informational, and neither tool fails all of them
by default. cargo-deny fails the unmaintained advisories. Its `unsound` scope defaults to
`"workspace"`, which fails only on a direct dependency of a workspace crate. On an `im` 15.1.0
tree it reports the `im` soundness advisory, but not the `sized-chunks` one that `im` pulls in.
`cargo audit` prints informational advisories as warnings and exits 0. Set `unsound = "all"`
under `[advisories]` in `deny.toml`, or pass `--deny warnings`:

```bash
cargo deny --config deny.toml --locked check advisories   # [advisories] unsound = "all"
cargo audit --deny warnings        # the same gate without cargo-deny
```

Then trace each flagged crate back to the dependency that pulls it in. Name it as
`name@version`; a partial version such as `@0.1` works. A plain name fails as ambiguous when the
lockfile holds two versions, for example `bitmaps` 2 under `im` and `bitmaps` 3 under `imbl`
7.0.1:

```bash
cargo tree --locked -i imbl-sized-chunks@0.1   # imbl 7.0.1
cargo tree --locked -i sized-chunks            # im 15.1.0
cargo tree --locked -i bitmaps@2               # im 15.1.0; bitmaps@3 is imbl 7.0.1
```

A match proves that the affected crate is in the resolved tree. It does not prove that your
code reaches the affected API. Do not claim reachability or exploitability without a call-path
audit.

If policy rejects the tree and no clean upgrade exists, keep `Vec`, select a dependency with a
clean resolved tree, or record a narrow temporary exception with an owner and an expiry. Do not
waive the advisory only because the direct dependency is maintained.

The `rust-security` skill, when it is installed, covers the `deny.toml` policy
(`unsound = "all"`) that turns a transitive unsound advisory, such as the `sized-chunks` one
under `im`, into a failed build.

## Review checklist

Check each change that adds or uses `im`, `imbl`, or `rpds`:

1. Is the collection read by index? Then it is the wrong structure. Keep `Vec` and clone it.
2. Does the write path use `&self -> Self` when only the newest version survives? Use the
   `&mut self` shape: `push_back_mut`, `set_mut`, `drop_last_mut` on `rpds` (8x fewer
   allocations), or `imbl` when write cost dominates (another factor of about 72).
3. Does the value cross a thread? `rpds::Vector::new()` is not `Send`. Use `new_sync()`.
4. Does the lockfile pass the advisory gate in
   [Check the resolved dependency tree](#check-the-resolved-dependency-tree)?

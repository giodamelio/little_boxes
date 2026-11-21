# Argument shapes

Read this file when you choose the parameter type for a `pub` or `pub(crate)` function that takes
text, a path, a sequence, or a callback. `SKILL.md` gives the default (`&str`, `&[T]`, `&Path`).
This file gives the acceptance set of every competing shape, so you can tell which caller you lock
out.

Measured on rustc 1.97.0, edition 2024, `aarch64-apple-darwin`, unless a section names rustc 1.98.1.

Contents:

- Acceptance matrix: text parameters
- Acceptance matrix: path parameters
- `Borrow<T>` is a map-key bound, never an argument bound
- `impl Into<String>` is narrower than `&str`
- Deref coercion never reaches a slice's elements
- `impl Trait` in argument position deletes the caller's turbofish
- A generic bound breaks fn-pointer coercion
- Callback bounds past the plain borrow
- Store a callback as `Box<dyn Fn>`, not `Box<dyn FnMut>`
- Monomorphisation cost, and the firewall
- When `&String` and `&Vec<T>` are correct
- The receiver is an argument too
- Triage

---

## Acceptance matrix: text parameters

The column is the type the caller already holds. The cell is the call the caller writes. `NO` means
no call form compiles without an extra conversion.

| Parameter | `&'static str` | `&String` | `String` | `Rc<str>`, `Arc<str>` | `Box<str>` | `Cow<'_, str>` | Bodies emitted |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `&String` | NO, E0308 | `f(x)` | `f(&x)` | NO, E0308 | NO, E0308 | NO, E0308 | 1 |
| `&str` | `f(x)` | `f(x)` | `f(&x)` | `f(&x)` | `f(&x)` | `f(&x)` | 1 |
| `impl AsRef<str>` | `f(x)` | `f(x)` | `f(x)` | `f(x)` | `f(x)` | `f(x)` | 1 per argument type |
| `impl Borrow<str>` | `f(x)` | **NO, E0277** | `f(x)`, moves | `f(x)`, moves | `f(x)`, moves | `f(x)`, moves | 1 per argument type |
| `impl Into<String>` | `f(x)`, allocates | `f(x)`, allocates | `f(x)`, moves | **NO, E0277** | `f(x)`, reuses the buffer | `f(x)` | 1 per argument type |
| `Cow<'_, str>` | `f(Cow::Borrowed(x))` | `f(Cow::Borrowed(x))` | `f(Cow::Owned(x))` | `f(Cow::Borrowed(&x))` | `f(Cow::Borrowed(&x))` | `f(x)` | 1 |
| `String` | `f(x.to_string())`, allocates | `f(x.clone())`, allocates | `f(x)` | `f(x.to_string())`, allocates | `f(x.into())`, reuses the buffer | `f(x.into_owned())` | 1 |

The `&str` row is full because deref coercion runs on the outer reference. `String`, `Rc<str>`,
`Arc<str>`, `Box<str>` and `Cow<'_, str>` all deref to `str`. That is a compiler coercion, not a
trait impl, which is why `impl Borrow<str>` and `impl Into<String>` have holes that the
`&str` row does not.

Buffer reuse is measured by pointer identity: `Box<str> -> String` and `Cow::Owned -> String` keep
the same `as_ptr()`; `Cow::Borrowed -> String` does not.

Default: take `&str`. Take `impl AsRef<str>` when call-site ergonomics matter more than one body
per argument type. Take `String` when you keep the value and the common caller can hand you one.

---

## Acceptance matrix: path parameters

| Parameter | `&'static str` | `String` | `&Path` | `PathBuf` | `&PathBuf` |
| --- | --- | --- | --- | --- | --- |
| `&Path` | `f(Path::new(x))` | `f(Path::new(&x))`; `f(&x)` is E0308 | `f(x)` | `f(&x)` | `f(x)` |
| `&PathBuf` | NO, E0308 | NO, E0308 | NO, E0308 | `f(&x)` | `f(x)` |
| `impl AsRef<Path>` | `f(x)` | `f(x)` | `f(x)` | `f(x)` | `f(x)` |
| `impl Borrow<Path>` | **NO, E0277** | **NO, E0277** | `f(x)` | `f(x)`, moves | **NO, E0277** |

`impl AsRef<Path>` is the only bound that takes every shape. `&Path` is the cheap default and only
costs the caller a `Path::new` on a string literal.

---

## `Borrow<T>` is a map-key bound, never an argument bound

`impl Borrow<Path>` looks like a stricter `AsRef<Path>`. It is a different set, not a subset. It
rejects `&str`, `String`, and — the case that surprises everyone — `&PathBuf`:

```text
error[E0277]: the trait bound `&PathBuf: Borrow<Path>` is not satisfied
   |
12 |     open(&pb);
   |          ^^^ the trait `Borrow<Path>` is not implemented for `&PathBuf`
help: consider dereferencing here
   |
12 |     open(&*pb);
help: consider removing the leading `&`-reference
```

The only blanket impl that applies is `impl<T: ?Sized> Borrow<T> for &T`. It gives
`&PathBuf: Borrow<PathBuf>` and stops there. `Borrow` deliberately does not chain through `Deref`,
because it carries a contract `AsRef` does not: the borrowed and the owned form must produce the
same `Hash` and the same `Ord`. `AsRef` has the chaining blanket impl
`impl<T: ?Sized, U: ?Sized> AsRef<U> for &T where T: AsRef<U>` and no such contract.

That contract is the whole point of `Borrow`, and it has exactly one use: generic lookup in a
`HashMap` or a `BTreeMap`.

```rust
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};

// Borrow is correct here: `map.get` needs K and Q to hash identically.
fn lookup<'m, K, Q, V>(map: &'m HashMap<K, V>, key: &Q) -> Option<&'m V>
where
    K: Borrow<Q> + Eq + Hash,
    Q: Eq + Hash + ?Sized,
{
    map.get(key)
}

fn main() {
    let mut by_path: HashMap<PathBuf, u32> = HashMap::new();
    by_path.insert(PathBuf::from("a"), 2);
    assert_eq!(lookup(&by_path, Path::new("a")), Some(&2));
}
```

Rule: `AsRef<T>` for arguments, `Borrow<T>` for map keys. Never the reverse.

---

## `impl Into<String>` is narrower than `&str`

`impl Into<String>` rejects `Rc<str>` and `Arc<str>`, which a plain `&str` parameter accepts:

```text
error[E0277]: the trait bound `String: From<Rc<str>>` is not satisfied
  = help: `String` implements trait `From<T>`:
            From<&String>
            From<&mut str>
            From<&str>
            From<Box<str>>
            From<Cow<'_, str>>
            From<char>
  = note: required for `Rc<str>` to implement `Into<String>`
```

That help text is the complete impl set. `Rc<str>` and `Arc<str>` are absent because unwrapping
either needs a refcount check plus a copy.

Rule: use `impl Into<String>` only when you keep the value *and* you want a `String` caller to
donate its allocation. Otherwise take `&str` and call `to_owned()` yourself. The caller who holds
an `Rc<str>` writes `f(&*x)`, which allocates, so the bound buys nothing over `&str` for them.

---

## Deref coercion never reaches a slice's elements

`&Vec<String>` coerces to `&[String]`. `&[String]` is **not** `&[&str]`. No unsizing rule and no
deref rule bridges the element type:

```text
error[E0308]: mismatched types
4 |     process_text(&lines);
  |     ------------ ^^^^^^ expected `&[&str]`, found `&Vec<String>`
```

The caller's only repair without a generic is
`.iter().map(String::as_str).collect::<Vec<_>>()` — one allocation plus one pointer write per
element, on every call. Do not put `&[&str]` or `&[&T]` in a public signature: it forces every
caller that holds owned strings to build a temporary `Vec`. Take `&[S]` under `S: AsRef<str>`:

```rust
use std::borrow::Cow;
use std::rc::Rc;

fn total_len<S: AsRef<str>>(lines: &[S]) -> usize {
    lines.iter().map(|line| line.as_ref().len()).sum()
}

fn main() {
    let owned: Vec<String> = vec!["a".into(), "bb".into()];
    let rcs: Vec<Rc<str>> = vec![Rc::from("a")];
    let cows: Vec<Cow<'_, str>> = vec![Cow::Borrowed("a")];
    let boxed: Vec<Box<str>> = vec![Box::from("a")];
    let array: [&str; 2] = ["a", "bb"];
    assert_eq!(total_len(&owned), 3);
    assert_eq!(total_len(&rcs), 1);
    assert_eq!(total_len(&cows), 1);
    assert_eq!(total_len(&boxed), 1);
    assert_eq!(total_len(&array), 3);
    assert_eq!(total_len::<&str>(&[]), 0);
}
```

Every one of those call sites is allocation-free.

---

## `impl Trait` in argument position deletes the caller's turbofish

Write the named form `<S: AsRef<str>>`, not `impl AsRef<str>`, in any signature a caller can reach
with an empty or inference-poor literal. `impl Trait` in argument position is an *anonymous*
generic parameter, so `f(&[])` is ambiguous and cannot be annotated:

```text
error[E0283]: type annotations needed
2 | fn main() { let _ = count(&[]); }
  |                     ----- ^^^ cannot infer type for type parameter `impl AsRef<str>`
  = note: multiple `impl`s satisfying `_: AsRef<str>` found in the following crates: `alloc`, `core`:
          - impl AsRef<str> for String;
          - impl AsRef<str> for str;
```

The escape hatch is closed too:

```text
error[E0107]: function takes 0 generic arguments but 1 generic argument was supplied
  = note: `impl Trait` cannot be explicitly specified as a generic argument
```

`<S: AsRef<str>>` infers identically and keeps `count::<&str>(&[])` available. The two forms are
not interchangeable in a public API; only one of them can be annotated by a caller.

---

## A generic bound breaks fn-pointer coercion

Changing a published `fn f(s: &str)` into `fn f(s: impl AsRef<str>)` or `fn f<S: AsRef<str>>(s: S)`
breaks every caller that coerces `f` to a `fn(&str)` pointer:

```text
error[E0308]: mismatched types
3 | fn main() { let _ = apply(generic, "ab"); }
  |                     ----- ^^^^^^^ one type is more general than the other
  = note: expected fn pointer `for<'a> fn(&'a str) -> _`
                found fn item `fn(_) -> _ {generic::<_>}`
```

A monomorphised fn item carries one concrete lifetime, never a higher-ranked one, so it cannot
satisfy `for<'a> fn(&'a str)`. An `Fn(&str)` bound is higher-ranked too, and fails with
`implementation of 'Fn' is not general enough` and no error code. Turbofishing the named form does
not help: `generic2::<&str>` pins `&'_ str` to a single inferred lifetime and gives the same E0308.
Only a wrapper closure works.

```rust
fn generic(s: impl AsRef<str>) -> usize {
    s.as_ref().len()
}

fn apply(f: fn(&str) -> usize, s: &str) -> usize {
    f(s)
}

fn main() {
    // `apply(generic, "ab")` is E0308. The closure is the only escape.
    assert_eq!(apply(|s: &str| generic(s), "ab"), 2);
}
```

The break needs a reference parameter and a higher-ranked target. `iter.map(f)` over `&str` items
still compiles, because each item has one concrete lifetime. A by-value parameter made generic,
`u8` to `T: Add<Output = T>`, still coerces to `fn(u8) -> u8`. Measured on rustc 1.98.1.

Rule: treat a change from a concrete parameter to any generic bound as a source-breaking change,
and ship it in a major version. This rule is stricter than the Cargo SemVer guide, which lists
"generalizing a function to use generics (supporting original type)" as a minor change and accepts
the type-inference failures it can cause. This rule counts two breakages: the fn-pointer and
`Fn`-bound breakage of a reference parameter, which the guide does not mention, and the inference
breakage of any parameter, which the guide accepts. cargo-semver-checks 0.50.0 does not flag the
change (measured on rustc 1.98.1). The `rust-crate-release` skill owns the final version-bump
decision; `cargo semver-checks` alone misses this break.

---

## Callback bounds past the plain borrow

`SKILL.md` gives the two base rules: `F: Fn(&T) -> R`, which elision already makes higher-ranked,
when the callback must not keep the reference, never a callback argument tied to a lifetime
parameter of the function or struct; and `F: Fn(&T) -> &K` with `K: ?Sized` when it returns a
borrow of its argument. This section holds the rest. Measured on rustc 1.98.1.

**Pick the bound from three separate axes.** `move` decides the capture mode. The body decides
the trait: a read-only body gives `Fn` even under `move`, and a body that moves a non-`Copy`
capture out gives `FnOnce`, whose second call fails with `E0382`. The captured types decide the
lifetime. A closure coerces to `fn` only with an empty capture set; one captured `i32` blocks it
with `E0308`.

**Declare a method lifetime on the method, or elide it.** `impl<'a> Doc { fn view(&'a self) ->
View<'a> }` is early bound, so `Doc::view` fails a `for<'x>` bound with `implementation of 'Fn' is
not general enough`: no error code, no suggested fix. `impl Doc { fn view(&self) -> View<'_> }` is
late bound and passes. A direct call compiles under both forms, so test the bound by passing the
method path itself.

**An output that stays a type parameter over `'a` needs a trait with a GAT.** Write
`type Out<'a>: Bound where Self: 'a, T: 'a;` and put the output bound on the GAT in the trait
definition. Two other shapes compile at the definition and fail at the first call with an element
type that borrows:

- a use-site bound `for<'a> K::Out<'a>: Bound`;
- a lifetime parameter on the trait (`for<'a> K: Proj<'a, T>`) together with an impl that states
  `'s: 'a`. Without that outlives bound the impl works.

Both give `error[E0597]: ... does not live long enough` with `note: due to a current limitation of
the type system, this implies a 'static lifetime`. Test the consumer with an element type that
borrows, because a `'static` element hides the failure. The `rust-callback-bounds` skill, when it
is installed, has the worked example of the `for<'a> K::Out<'a>: Bound` failure.

---

## Store a callback as `Box<dyn Fn>`, not `Box<dyn FnMut>`

Each closure expression has its own anonymous type, so a `Vec<F>` cannot hold closures from two
expressions, and `impl Fn` in a field fails with `E0562`. `Box<dyn FnMut(..)>` needs a unique
borrow at the call, so `publish(&self)` fails with `E0596` and must become `&mut self`, which
propagates to every caller and blocks sharing the owner behind an `Arc`.

```rust
pub struct Bus {
    // `dyn Fn` keeps `publish(&self)`. `dyn FnMut` forces `publish(&mut self)`.
    subs: Vec<Box<dyn Fn(u32)>>,
}
impl Bus {
    pub fn subscribe(&mut self, f: impl Fn(u32) + 'static) { self.subs.push(Box::new(f)); }
    pub fn publish(&self, ev: u32) { for s in &self.subs { s(ev); } }
}
```

The field carries an implicit `+ 'static` bound. Clone the captured data; do not add a lifetime
parameter to the owner, and do not store `Weak` callbacks to dodge it: a `Weak` registration dies
silently when the caller releases its `Arc`.

---

## Monomorphisation cost, and the firewall

Every distinct argument type gets its own copy of the whole function body. Measured at
`-C opt-level=3` with one `impl AsRef<str>` function that scans its argument, called on `&str`,
`String`, `Rc<str>` and `Box<str>`: four bodies of 171, 177, 182 and 186 instructions — the same
loop, emitted four times.

Split the signature from the work. The generic wrapper collapses to a coercion, and the body exists
once:

```rust
// Public, generic, tiny: it only calls `as_ref` and tail-calls.
pub fn count_x(text: impl AsRef<str>) -> usize {
    count_x_inner(text.as_ref())
}

// Private, concrete, holds the whole body.
fn count_x_inner(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'x').count()
}
```

With that split the same four instantiations measured 1, 16, 17 and 20 instructions, and they
share one copy of the body. The standard library uses this shape everywhere: `File::open` is
`pub fn open<P: AsRef<Path>>(path: P) -> io::Result<File>` whose whole body is
`OpenOptions::new().read(true).open(path.as_ref())`. Apply it to every `impl AsRef<_>` parameter on
a function longer than a few lines.

---

## When `&String` and `&Vec<T>` are correct

The rule "never take `&String` or `&Vec<T>`" is stated as absolute and it is not. Two of the three
usual justifications for it are wrong, and two exceptions are real.

**Wrong justification 1: "my callers all own a `Vec`, so `&Vec<T>` costs them nothing."** Owning a
`Vec` does not mean wanting to pass all of it. A caller who owns the exact vector still cannot pass
a sub-range, an array, or a boxed slice:

```text
error[E0308]: mismatched types
4 |     let _ = sum_vec(&v[1..]);
  |             ------- ^^^^^^^ expected `&Vec<u32>`, found `&[u32]`
5 |     let _ = sum_vec(&[1u32, 2, 3]);
  |             ------- ^^^^^^^^^^^^^ expected `&Vec<u32>`, found `&[u32; 3]`
6 |     let _ = sum_vec(&vec![1u32].into_boxed_slice());
  |             ------- expected `&Vec<u32>`, found `&Box<[u32]>`
```

Indexing a `Vec` with a range yields `[T]`, and there is no coercion from `&[T]` back up to
`&Vec<T>`.

**Wrong justification 2: "I have to clone it, so I need the owned type."** `clone()` on a `&Vec<T>`
allocates exactly `len`, not `capacity`. Measured with a source of capacity 100 and length 1:
`v.clone()` produces capacity 1, and `v.as_slice().to_vec()` also produces capacity 1. The same
holds for `String`: from a source of capacity 100 and length 3, both `s.clone()` and
`s.as_str().to_owned()` produce capacity 3. The two paths are identical, so cloning is never a
reason to widen the parameter.

**Real exception: the body needs a method the slice does not have.** `&str` and `&[T]` expose no
allocation state:

```text
error[E0599]: no method named `capacity` found for reference `&str` in the current scope
error[E0599]: no method named `capacity` found for reference `&[u32]` in the current scope
```

Take `&Vec<T>` or `&String` when the body only reads allocation state with `capacity`. Take
`&mut Vec<T>` or `&mut String` when the body calls `reserve`, `push`, `truncate`, or
`shrink_to_fit`. `into_boxed_slice` takes `self`, so it needs the owned `Vec<T>`. Clippy already encodes this:
`clippy::ptr_arg` is body-aware and stays silent on `fn(v: &Vec<u32>) -> usize { v.capacity() }`
and on `fn(v: &mut Vec<u32>) { v.push(1); }`, while it fires on
`fn(v: &mut Vec<u32>) { v.sort(); }` with `writing '&mut Vec' instead of '&mut [_]' involves a new
object where a slice will do`.

**Second real exception: container identity.** Two distinct empty `Vec`s share a dangling data
pointer, so `std::ptr::eq(a.as_slice(), b.as_slice())` is `true` while `std::ptr::eq(&a, &b)` is
`false`. When the function compares container identity, the slice cannot express it.

Write the justification in a comment next to any surviving `&String` or `&Vec<T>` parameter. A
reviewer must not have to re-derive it.

---

## The receiver is an argument too

A consuming setter `fn with_x(self) -> Self` is unreachable from any caller who holds `&mut Self`,
including the struct's own `&mut self` methods when the value is a field:

```text
error[E0507]: cannot move out of `self.cfg` which is behind a mutable reference
5 | impl Holder { fn bump(&mut self) { self.cfg = self.cfg.with_retries(7); } }
  |                                               ^^^^^^^^ --------------- `self.cfg` moved due to this method call
note: `Cfg::with_retries` takes ownership of the receiver `self`, which moves `self.cfg`
help: consider cloning the value if the performance cost is acceptable
  |
5 | ... self.cfg = self.cfg.clone().with_retries(7); ...
```

Do not take that `help`. The `.clone()` it proposes reintroduces the allocation the consuming
setter was meant to avoid. Use `mem::take`, or ship a `&mut self` setter next to the consuming one.

```rust
#[derive(Default, Clone, Debug)]
struct Cfg {
    retries: u32,
}

impl Cfg {
    fn with_retries(mut self, count: u32) -> Self {
        self.retries = count;
        self
    }

    fn set_retries(&mut self, count: u32) -> &mut Self {
        self.retries = count;
        self
    }
}

struct Holder {
    cfg: Cfg,
}

impl Holder {
    // `mem::take` leaves a valid default in the hole, so the move is legal and free.
    fn bump(&mut self) {
        let old = std::mem::take(&mut self.cfg);
        self.cfg = old.with_retries(7);
    }

    // Or reach for the borrowing setter and skip the dance.
    fn bump_direct(&mut self) {
        self.cfg.set_retries(7);
    }
}

fn main() {
    let mut holder = Holder { cfg: Cfg::default() };
    holder.bump();
    holder.bump_direct();
    assert_eq!(holder.cfg.retries, 7);
}
```

The receiver-shape table for builders, and the E0382 that a consuming setter gives inside a loop,
are in [ownership-and-concurrency-traps.md](ownership-and-concurrency-traps.md).

---

## Triage

| Symptom | Cause | Fix |
| --- | --- | --- |
| `E0308: expected '&Vec<u32>', found '&[u32]'` | parameter is `&Vec<T>` | take `&[T]` |
| `E0308: expected '&[&str]', found '&Vec<String>'` | parameter is a slice of borrows | take `&[S]` under `S: AsRef<str>` |
| `E0277: the trait bound '&PathBuf: Borrow<Path>' is not satisfied` | `Borrow` used as an argument bound | take `impl AsRef<Path>` |
| `E0277: the trait bound 'String: From<Rc<str>>' is not satisfied` | parameter is `impl Into<String>` | take `&str`, or have the caller write `&*x` |
| `E0283: cannot infer type for type parameter 'impl AsRef<str>'` | `impl Trait` argument plus an empty literal | declare `<S: AsRef<str>>` |
| `E0107: 'impl Trait' cannot be explicitly specified as a generic argument` | caller tried to turbofish an `impl Trait` argument | declare `<S: AsRef<str>>` |
| `E0308: one type is more general than the other`, `found fn item` | generic fn passed where a `fn` pointer is expected | wrap in a closure, or keep the concrete parameter |
| `implementation of 'Fn' is not general enough` on a method path | method lifetime declared on the `impl` block (early bound) | declare it on the method, or elide it |
| `E0597` with `this implies a 'static lifetime` at a generic callback | use-site `for<'a>` bound on a GAT, or a trait lifetime tied by `'s: 'a` | put the output bound on the GAT in the trait definition |
| `E0507: cannot move out of 'self.cfg' which is behind a mutable reference` | consuming setter called on a field | `std::mem::take`, or a `&mut self` setter |
| `E0599: no method named 'capacity' found for reference '&[u32]'` | the body needs the owned type | keep `&Vec<T>` and write down why |

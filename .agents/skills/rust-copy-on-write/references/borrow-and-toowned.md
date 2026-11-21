# `Borrow` and `ToOwned`, the layer under `Cow`

Deep material for `rust-copy-on-write`. Read it before you write `impl Borrow<..> for ..`,
before you give a type its own owned representation, and when a `HashMap::get` misses a key the
map holds.

Contents:

- [`Borrow` is a promise. `AsRef` is not](#borrow-is-a-promise-asref-is-not)
- [Break the contract and the lookup silently misses](#break-the-contract-and-the-lookup-silently-misses)
- [Fix: put `Hash` and `Eq` on the borrowed half](#fix-put-hash-and-eq-on-the-borrowed-half)
- [`self.borrow()` stops resolving: E0283](#selfborrow-stops-resolving-e0283)
- [`&String` does not implement `Borrow<str>`](#string-does-not-implement-borrowstr)
- [A `Clone` type cannot customise `ToOwned`](#a-clone-type-cannot-customise-toowned)
- [When a dependency demands `&String`](#when-a-dependency-demands-string)
- [`into_static`: the exit from a borrowing struct](#into_static-the-exit-from-a-borrowing-struct)
- [Checklist](#checklist)
- [Slice comparisons on Rust 1.100+](#slice-comparisons-on-rust-1100)

`Cow<'a, B>` is declared over `B: ToOwned`, and `ToOwned` requires `Owned: Borrow<B>`. Both
traits carry a contract that the compiler never checks. Break the `Borrow` one and you get a
silent wrong answer, not a compile error.

Every number below was measured on rustc 1.98.1, edition 2024, aarch64-apple-darwin, release
profile, with the allocator in [counting-allocator.md](counting-allocator.md).

## `Borrow` is a promise. `AsRef` is not

| | `AsRef<T>` | `Borrow<T>` |
| --- | --- | --- |
| Contract | none beyond "hands out a `&T`" | `Hash`, `Eq` and `Ord` of `Self` and of `T` must give identical results |
| Composes through `&` | yes: `&String: AsRef<str>` holds | no: `&String: Borrow<str>` does not hold |
| std uses it for | argument conversion (`Path::new`, `File::open`) | `HashMap::get`, `BTreeMap::get`, `HashSet::contains`, `Cow` |
| Break the contract | nothing happens | `get` returns `None` for a key the map contains |

Take `&str` for an argument you only read, or `impl AsRef<str>` when call-site convenience
outweighs one body per argument type (the `rust-discipline` skill owns that choice). Take
`Borrow<str>` only when a collection must look the value up by its borrowed form. The bound is
not a style choice: it is the point at which you accept the hash-equivalence obligation.

## Break the contract and the lookup silently misses

This compiles, never panics, and returns the wrong answer. `CiKey` hashes case-folded, but
`Borrow<str>` hands out the raw `str`, whose `Hash` is case-sensitive:

```rust
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

struct CiKey(String);

impl PartialEq for CiKey {
    fn eq(&self, o: &Self) -> bool { self.0.eq_ignore_ascii_case(&o.0) }
}
impl Eq for CiKey {}
impl Hash for CiKey {
    fn hash<H: Hasher>(&self, h: &mut H) { self.0.to_ascii_lowercase().hash(h) }
}
impl Borrow<str> for CiKey {
    fn borrow(&self) -> &str { &self.0 }          // contract broken here
}

fn main() {
    let mut m: HashMap<CiKey, u32> = HashMap::new();
    m.insert(CiKey("Content-Type".into()), 1);

    println!("{:?}", m.get("Content-Type"));                       // None
    println!("{:?}", m.contains_key(&CiKey("Content-Type".into()))); // true
}
```

`get::<str>` hashes the query with `str::hash` and probes that bucket. The entry was placed with
`CiKey::hash`. The probe lands in the wrong bucket, and the equality check never runs.

Measured over 2000 process runs of that binary: `get("Content-Type")` returned `Some` 13 times
and `None` 1987 times. `RandomState` reseeds per process, so an occasional hash coincidence makes
the lookup succeed. A lookup through the owned key, such as the `contains_key` above, always
succeeds. A test suite that looks up only through the owned key never sees the defect, and that
is how it reaches production. Test every `get` path through the borrowed form.

A `get` test alone still passes in about 1 run in 150 with the broken contract. Add a
deterministic check with `BuildHasher::hash_one` (stable since 1.71). Hash the owned key and its
borrowed form with one `RandomState`, and require equal results. Check a key in each letter case
that `eq` folds. An all-lowercase `CiKey` hashes the same both ways and hides the defect:

```rust,run
use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};

struct CiKey(String);

impl Hash for CiKey {
    fn hash<H: Hasher>(&self, h: &mut H) { self.0.to_ascii_lowercase().hash(h) }
}
impl Borrow<str> for CiKey {
    fn borrow(&self) -> &str { &self.0 }
}

fn hashes_agree(key: &CiKey) -> bool {
    let s = RandomState::new();
    s.hash_one(key) == s.hash_one(Borrow::<str>::borrow(key))
}

fn main() {
    assert!(hashes_agree(&CiKey("content-type".into())));    // hides the defect
    assert!(!hashes_agree(&CiKey("Content-Type".into())));   // finds it on every run
}
```

No tool reports it. `cargo clippy -- -W clippy::all -W clippy::pedantic` on that file prints
nothing, and there is no `debug_assert` inside `HashMap` for it.

## Fix: put `Hash` and `Eq` on the borrowed half

The owned type delegates to the borrowed type, so the two cannot disagree. The borrowed half is
an unsized `#[repr(transparent)]` newtype:

```rust,run
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[repr(transparent)]
pub struct CiStr(str);

impl CiStr {
    pub fn new(s: &str) -> &CiStr {
        // SAFETY: CiStr is #[repr(transparent)] over str, so the two have the
        // same layout and the same metadata.
        unsafe { &*(s as *const str as *const CiStr) }
    }
}

impl PartialEq for CiStr {
    fn eq(&self, o: &Self) -> bool { self.0.eq_ignore_ascii_case(&o.0) }
}
impl Eq for CiStr {}
impl Hash for CiStr {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for b in self.0.as_bytes() { h.write_u8(b.to_ascii_lowercase()); }
        h.write_u8(0xff);                      // terminator; see below
    }
}

pub struct CiString(String);

impl PartialEq for CiString {
    fn eq(&self, o: &Self) -> bool {
        Borrow::<CiStr>::borrow(self) == Borrow::<CiStr>::borrow(o)
    }
}
impl Eq for CiString {}
impl Hash for CiString {
    fn hash<H: Hasher>(&self, h: &mut H) { Borrow::<CiStr>::borrow(self).hash(h) }
}
impl Borrow<CiStr> for CiString {
    fn borrow(&self) -> &CiStr { CiStr::new(&self.0) }
}

fn main() {
    let mut m: HashMap<CiString, u32> = HashMap::new();
    m.insert(CiString("Content-Type".into()), 1);
    assert_eq!(m.get(CiStr::new("CONTENT-TYPE")), Some(&1));
    assert_eq!(m.get(CiStr::new("content-type")), Some(&1));
    assert_eq!(m.get(CiStr::new("other")), None);
}
```

Two details that a hand-written `Hash` gets wrong:

- Write a terminator or a length. std writes a terminator after every `str`, so `("ab", "c")`
  and `("a", "bc")` hash differently. A bare `Hasher::write` over the raw bytes with no
  separator makes them collide; a probe binary confirms it in three lines.
- Hash exactly the bytes that `eq` compares. Case-folding in `eq` and not in `hash` is the same
  defect as above, one level down.

Add `Ord` and `PartialOrd` to the pair as well when the key goes into a `BTreeMap`. `BTreeMap`
uses `Ord`, not `Hash`, and the same equivalence rule applies. Write `partial_cmp` as
`Some(self.cmp(other))`, so the two orderings cannot disagree.

## `self.borrow()` stops resolving: E0283

The moment your type has any `impl Borrow<X> for T`, the method call `self.borrow()` becomes
ambiguous. Core supplies `impl<T: ?Sized> Borrow<T> for T`, so two impls always apply:

```text
error[E0283]: type annotations needed
   |
33 |         self.borrow() == o.borrow()
   |              ^^^^^^
   |
note: multiple `impl`s satisfying `CiString: Borrow<_>` found
   |
40 | impl Borrow<CiStr> for CiString {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = note: and another `impl` found in the `core` crate:
           - impl<T> Borrow<T> for T
             where T: ?Sized;
```

This is an inference failure, not a coherence error. Write `Borrow::<CiStr>::borrow(self)` or
`<CiString as Borrow<CiStr>>::borrow(self)` in every delegating body. Expect it in `eq`, `hash`,
`cmp`, and `Deref::deref`.

## `&String` does not implement `Borrow<str>`

`Borrow` impls do not compose, and deref coercion does not apply to a trait bound. std ships
`impl Borrow<str> for String`, `impl<T: ?Sized> Borrow<T> for T`, and
`impl<T: ?Sized> Borrow<T> for &T`. The last one gives `&String: Borrow<String>`, never
`&String: Borrow<str>`.

```rust,run
use std::borrow::Borrow;
use std::collections::HashMap;

fn lookup<K: Borrow<str>>(_k: K) {}

fn main() {
    let s = String::from("a");
    lookup(s.clone());      // String: ok
    lookup("a");            // &str:   ok
    lookup(&*s);            // &str again: the fix rustc suggests
    // lookup(&s);          // &String: error[E0277]

    // No `impl Borrow<(&str, &str)> for (String, String)` exists, so a
    // tuple-keyed map cannot be queried from borrowed halves.
    let mut m: HashMap<(String, String), u32> = HashMap::new();
    m.insert(("a".into(), "b".into()), 1);
    // m.get(&("a", "b"));  // error[E0308]: expected `String`, found `&str`
    assert_eq!(m.get(&("a".to_string(), "b".to_string())), Some(&1));
}
```

`&String` is the shape callers hold most often, so `K: Borrow<str>` in a public signature turns
into `E0277` at the call site with the note `the trait Borrow<str> is not implemented for
&String`. Take `&str` or `impl AsRef<str>` unless the `Hash` guarantee is the reason the bound
exists.

The tuple case has no fix inside `Borrow`. A `HashMap<(String, String), V>` cannot be probed
with `(&str, &str)`. Five exits, in order of preference:

| Exit | Cost |
| --- | --- |
| Key the map on `(Cow<'static, str>, Cow<'static, str>)` and probe with `&(Cow::Borrowed(a), Cow::Borrowed(b))` | zero allocation per lookup. Inserts store `Cow::Owned`, or `Cow::Borrowed` for a literal |
| With `hashbrown` or `indexmap`, define a local `struct Pair<'a>(&'a str, &'a str)` with `#[derive(Hash)]` (it hashes like the owned tuple) and implement `equivalent::Equivalent<(String, String)>` for it; the orphan rule rejects the impl on a bare `(&str, &str)` | zero allocation per lookup; the same hash-equivalence contract as `Borrow` |
| Nest the maps: `HashMap<String, HashMap<String, V>>` | two probes per lookup, zero allocation |
| Join the halves into one `String` key with a separator neither half can contain, then probe it through `Borrow<str>` from a reused join buffer | two `push_str` calls per lookup, zero steady-state allocation |
| Build the owned tuple for every probe | two allocations per lookup on a 2-tuple |

The `Cow` key works because `HashMap` is covariant in its key: a `&HashMap<(Cow<'static, str>, ..), V>`
shrinks to the probe's lifetime, so the probe halves need not be `'static`. The returned `&V`
then lives only as long as the probe strings; copy or clone the value out when it must outlive
them.

```rust,run
use std::borrow::Cow;
use std::collections::HashMap;

type Key = (Cow<'static, str>, Cow<'static, str>);

fn find(m: &HashMap<Key, u32>, a: &str, b: &str) -> Option<u32> {
    m.get(&(Cow::Borrowed(a), Cow::Borrowed(b))).copied()
}

fn main() {
    let mut m: HashMap<Key, u32> = HashMap::new();
    m.insert((Cow::Owned(String::from("a")), Cow::Borrowed("b")), 1);
    let (a, b) = (String::from("a"), String::from("b"));
    assert_eq!(find(&m, &a, &b), Some(1));
}
```

## A `Clone` type cannot customise `ToOwned`

`alloc` ships `impl<T: Clone> ToOwned for T { type Owned = T; }`. It is in another crate, so it
cannot be removed, and coherence forbids a second impl for any `Clone` type:

```text
error[E0119]: conflicting implementations of trait `ToOwned` for type `Token`
  |
7 | impl std::borrow::ToOwned for Token {
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: conflicting implementation in crate `alloc`:
          - impl<T> ToOwned for T
            where T: Clone;
```

An unsized type has no `Clone` impl, so the blanket impl cannot reach it. That is exactly the
shape std uses for every real `Cow` target: `str`/`String`, `Path`/`PathBuf`, `[T]`/`Vec<T>`,
`OsStr`/`OsString`, `CStr`/`CString`. Copy it:

```rust,run
use std::borrow::{Borrow, Cow, ToOwned};

#[repr(transparent)]
pub struct Ascii(str);

impl Ascii {
    pub fn new(s: &str) -> &Ascii {
        // SAFETY: Ascii is #[repr(transparent)] over str.
        unsafe { &*(s as *const str as *const Ascii) }
    }
    pub fn len(&self) -> usize { self.0.len() }
}

pub struct AsciiBuf(String);

impl ToOwned for Ascii {
    type Owned = AsciiBuf;
    fn to_owned(&self) -> AsciiBuf { AsciiBuf(self.0.to_owned()) }
}
impl Borrow<Ascii> for AsciiBuf {
    fn borrow(&self) -> &Ascii { Ascii::new(&self.0) }
}

fn measure(c: Cow<'_, Ascii>) -> usize { c.len() }

fn main() {
    assert_eq!(measure(Cow::Borrowed(Ascii::new("hello"))), 5);   // 0 allocations
    assert_eq!(measure(Cow::Owned(AsciiBuf("world!".into()))), 6);
}
```

The pair costs one `unsafe` transmute helper. Keep it in one `new` method with a `SAFETY:`
comment, and never write the cast at a use site. `#[repr(transparent)]` gives `Ascii` the
layout of `str`. The cast `s as *const str as *const Ascii` keeps the length metadata, so the
reborrow is sound (Reference, type layout, `repr(transparent)`). The `rust-unsafe` skill, when
it is installed, covers the `SAFETY:` comment and audit rules.

The same blanket impl is why `Cow<'_, String>` type-checks and is useless. `SKILL.md` holds that
rule, the `owned_cow` lint, and the grep.

## When a dependency demands `&String`

A legacy signature `fn legacy(s: &String)` cannot take your `&str`, and you cannot change the
crate. The circulating answer is a `ManuallyDrop` plus `String::from_raw_parts` fabrication that
builds a fake `String` over borrowed bytes. Do not write it: it buys almost nothing. The
`rust-unsafe` skill, when it is installed, owns the reason it is undefined behavior:
`String::from_raw_parts` requires a buffer from the global allocator, and Miri can pass the
fabrication anyway.

Reuse one buffer instead. `String::clear` keeps the capacity, so `push_str` stops reallocating
after the first few calls:

```rust,run
fn legacy(s: &String) -> usize { s.len() }

// 5000 allocations over 5000 calls.
fn naive(inputs: &[&str], n: usize) -> usize {
    let mut sum = 0;
    for _ in 0..n {
        for s in inputs { sum += legacy(&s.to_string()); }
    }
    sum
}

// 2 allocations over 5000 calls. No unsafe.
fn reuse(inputs: &[&str], n: usize) -> usize {
    let mut sum = 0;
    let mut buf = String::new();
    for _ in 0..n {
        for s in inputs {
            buf.clear();
            buf.push_str(s);
            sum += legacy(&buf);
        }
    }
    sum
}

fn main() {
    let inputs = ["alpha", "beta", "gamma-long-value", "d", "epsilon"];
    assert_eq!(naive(&inputs, 1000), reuse(&inputs, 1000));
}
```

Measured with the counting allocator over 5 inputs x 1000 iterations:

| Shape | Allocations |
| --- | --- |
| `legacy(&s.to_string())` per call | 5000 |
| Reused `String` buffer | 2 |

The `ManuallyDrop` fabrication allocates nothing by construction. Its whole advantage over the
reused buffer is those two warm-up allocations, which amortise to nothing.
Hoist the buffer to the widest scope the call site allows. When the calls run on several
threads, give each thread its own buffer with `thread_local!`, not a shared `Mutex<String>`.

## `into_static`: the exit from a borrowing struct

A struct with a `Cow<'a, str>` field cannot become `'static` while a field borrows the parse
buffer. Convert it where the buffer dies, at one allocation per still-borrowed field:

```rust,run
use std::borrow::Cow;

pub struct Header<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
}

impl Header<'_> {
    /// One allocation per borrowed field. Call it where the parse buffer dies.
    pub fn into_static(self) -> Header<'static> {
        Header {
            name: Cow::Owned(self.name.into_owned()),
            value: Cow::Owned(self.value.into_owned()),
        }
    }
}

fn main() {
    let raw = String::from("host: example");
    let (n, v) = raw.split_once(": ").expect("the literal holds the separator");
    let owned = Header { name: Cow::Borrowed(n), value: Cow::Borrowed(v) }.into_static();
    let store: Vec<Header<'static>> = vec![owned];
    drop(raw);
    assert_eq!(store[0].name, "host");
}
```

## Checklist

1. Does any type in the crate implement `Borrow<X>`? Then its `Hash`, `Eq` and `Ord` must
   delegate to `X`. Check every one, and pin `Hash` with the `hash_one` test.
2. Does a custom `Hash` skip a length or a terminator between fields? Add one.
3. Does a public bound say `K: Borrow<str>` where `impl AsRef<str>` would do? `&String` fails
   the first and passes the second.
4. Does a delegating `eq` or `hash` call `self.borrow()`? Qualify it, or it is `E0283`.
5. Does a call site build a `String` only to satisfy a `&String` parameter? Reuse one buffer.
6. Does the fix reach for `ManuallyDrop` plus `String::from_raw_parts`? Reject it in review.

## Slice comparisons on Rust 1.100+

Rust 1.100 lets `Vec<T>`, `&[T]`, and `&mut [T]` compare directly with
`Cow<'_, [U]>` when the elements support the required comparison. Do not call
`into_owned` only to compare; that can copy a borrowed buffer.

```rust,run
use std::borrow::Cow;

fn main() {
    let values = vec![1_u8, 2, 3];
    let borrowed: Cow<'_, [u8]> = Cow::Borrowed(&[1, 2, 3]);
    assert!(values == borrowed);
    assert!(values.as_slice() == borrowed);
    let mut array = [1_u8, 2, 3];
    assert!(&mut array[..] == borrowed);
    assert!(matches!(borrowed, Cow::Borrowed(_)));
}
```

This probe checks comparison and preserves the borrowed variant. It is not an
allocator measurement. On an older MSRV, compare slices with `cow.as_ref()`.

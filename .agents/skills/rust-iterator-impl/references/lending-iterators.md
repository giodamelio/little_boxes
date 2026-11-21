# Lending iterators: shapes that compile

Deep material for `rust-iterator-impl`. Read it after E0207, when an iterator must yield
references into data it owns. Every block below compiles on rustc 1.98.1, edition 2024.

Contents:

- [The E0207 output and its misleading help](#the-e0207-output-and-its-misleading-help)
- [1. Borrow the data, do not own it](#1-borrow-the-data-do-not-own-it)
- [2. Yield a guard that owns its borrow](#2-yield-a-guard-that-owns-its-borrow)
- [3. Drop the Iterator impl and expose an inherent method](#3-drop-the-iterator-impl-and-expose-an-inherent-method)
- [A lending trait with a generic associated type](#a-lending-trait-with-a-generic-associated-type)

## The E0207 output and its misleading help

`Item` is an associated type on the impl, so it cannot name the lifetime of `&mut self`:

```text
error[E0207]: the lifetime parameter `'a` is not constrained by the impl trait, self type, or predicates
2 | impl<'a> Iterator for Chunks {
  |      ^^ unconstrained lifetime parameter
help: use the lifetime parameter `'a` in the `Chunks` type and use it in the type definition
error: lifetime may not live long enough
7 |         Some(s)
  |         ^^^^^^^ method was supposed to return data with lifetime `'a` but it is returning data with lifetime `'1`
```

Both errors have one cause. The `help` works only for option 1 below, where a field borrows
with `'a`. Following it on a struct that owns its buffer adds
`error[E0392]: lifetime parameter 'a is never used`, and the lifetime error stays. A
`PhantomData<&'a ()>` field removes the E0392 only. No attribute and no bound fixes this shape.

## 1. Borrow the data, do not own it

The lifetime goes on the iterator struct, so the impl constrains it. The E0207 `help` line
leads here:

```rust,run
pub struct Frames<'a> { data: &'a [u8], pos: usize }

impl<'a> Iterator for Frames<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        let end = (self.pos + 4).min(self.data.len());
        let frame = self.data.get(self.pos..end)?;
        self.pos = end;
        if frame.is_empty() { None } else { Some(frame) }
    }
}

fn main() {
    let buf = [0u8; 10];
    assert_eq!(Frames { data: &buf, pos: 0 }.count(), 3);
}
```

## 2. Yield a guard that owns its borrow

The item is `Ref<'a, T>`, and `'a` comes from the container the iterator borrows:

```rust
use std::cell::{Ref, RefCell};

pub struct NodeIter<'a> { cells: &'a [RefCell<u32>], pos: usize }

impl<'a> Iterator for NodeIter<'a> {
    type Item = Ref<'a, u32>;
    fn next(&mut self) -> Option<Ref<'a, u32>> {
        let cell = self.cells.get(self.pos)?;
        self.pos += 1;
        Some(cell.borrow())
    }
}
```

## 3. Drop the Iterator impl and expose an inherent method

The elided lifetime ties the item to `&mut self`, which is exactly what `Iterator` cannot
express. The caller loses every adapter and uses `while let`:

```rust,run
pub struct Decoder { buf: Vec<u8>, pos: usize }

impl Decoder {
    /// Not an `Iterator`: the item borrows `self`, so no `Item` type can name it.
    pub fn next_frame(&mut self) -> Option<&[u8]> {
        let end = (self.pos + 4).min(self.buf.len());
        let frame = self.buf.get(self.pos..end).filter(|f| !f.is_empty())?;
        self.pos = end;
        Some(frame)
    }
}

fn main() {
    let mut d = Decoder { buf: vec![1, 2, 3, 4, 5], pos: 0 };
    let mut seen = 0;
    while let Some(frame) = d.next_frame() { seen += frame.len(); }
    assert_eq!(seen, 5);
}
```

## A lending trait with a generic associated type

A generic associated type expresses option 3 as a trait on stable. It gets no `for` loop and no
std adapters, so write one only when generic code must accept several lending sources:

```rust,run
pub trait LendingIterator {
    type Item<'a> where Self: 'a;
    fn next(&mut self) -> Option<Self::Item<'_>>;
}

pub struct Pairs { buf: Vec<u8>, pos: usize }

impl LendingIterator for Pairs {
    type Item<'a> = &'a mut [u8];
    fn next(&mut self) -> Option<&mut [u8]> {
        let end = (self.pos + 2).min(self.buf.len());
        if self.pos >= end { return None; }
        let chunk = &mut self.buf[self.pos..end];
        self.pos = end;
        Some(chunk)
    }
}

fn main() {
    let mut p = Pairs { buf: vec![1, 2, 3], pos: 0 };
    let mut n = 0;
    while let Some(chunk) = p.next() { chunk[0] = 0; n += 1; }
    assert_eq!((n, p.buf), (2, vec![0, 2, 0]));
}
```

A generic consumer that bounds the item with `for<'a> L::Item<'a>: Trait` forces `L: 'static`.
On a source that borrows, such as `Pairs<'b> { buf: &'b mut [u8], pos: usize }`, rustc 1.98.1
gives `error[E0597]: ... does not live long enough` with
`note: due to a current limitation of the type system, this implies a 'static lifetime`. Put the
bound on the GAT in the trait definition: `type Item<'a>: Debug where Self: 'a;`. Test the
consumer with a source that borrows, because `Pairs` above owns its `Vec`, is `'static`, and
hides the failure.

The trait method is named `next`, which does not trip `should_implement_trait`: that lint checks
inherent methods only.

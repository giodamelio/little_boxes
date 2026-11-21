# Run-tested iterator impls

Deep material for [SKILL.md](../SKILL.md): the complete programs behind its rules. Each one is a
`rust,run` probe that CI compiles and runs on the native host.

Contents:

- [The three IntoIterator impls on one container](#the-three-intoiterator-impls-on-one-container)
- [A collection newtype with four impls](#a-collection-newtype-with-four-impls)
- [An exact-size, double-ended iterator](#an-exact-size-double-ended-iterator)

## The three IntoIterator impls on one container

A wrapper iterator that forwards `size_hint` and re-implements each trait of the inner iterator,
the inherent `iter` and `iter_mut`, and the three `IntoIterator` impls that delegate to them.
The `&mut` impl names `std::slice::IterMut` to keep the block short; wrap it like the shared
iterator in a published crate.

```rust,run
pub struct Bag { items: Vec<u32> }

#[derive(Clone, Debug)]
pub struct Iter<'a>(std::slice::Iter<'a, u32>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a u32;
    fn next(&mut self) -> Option<&'a u32> { self.0.next() }
    // Forward it. The default is (0, None).
    fn size_hint(&self) -> (usize, Option<usize>) { self.0.size_hint() }
}

// Re-implement each trait the inner iterator has.
impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> { self.0.next_back() }
}
impl ExactSizeIterator for Iter<'_> {}
impl std::iter::FusedIterator for Iter<'_> {}

impl Bag {
    pub fn iter(&self) -> Iter<'_> { Iter(self.items.iter()) }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.items.iter_mut() }
}

impl<'a> IntoIterator for &'a Bag {
    type Item = &'a u32;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> { self.iter() }
}

impl<'a> IntoIterator for &'a mut Bag {
    type Item = &'a mut u32;
    type IntoIter = std::slice::IterMut<'a, u32>;
    fn into_iter(self) -> Self::IntoIter { self.iter_mut() }
}

impl IntoIterator for Bag {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;
    // Name the concrete constructor. Never `self.into_iter()`.
    fn into_iter(self) -> Self::IntoIter { self.items.into_iter() }
}

fn total<I: IntoIterator<Item = u32>>(src: I) -> u32 { src.into_iter().sum() }

fn main() {
    let mut bag = Bag { items: vec![1, 2, 3] };
    for v in &bag { assert!(*v > 0); }
    for v in &mut bag { *v += 1; }
    let mut it = bag.iter();
    it.next();
    assert_eq!(it.size_hint(), (2, Some(2)));
    assert_eq!((it.len(), it.next_back()), (2, Some(&4)));
    assert_eq!(total(bag), 9);
}
```

## A collection newtype with four impls

`IntoIterator` by value and by reference, `FromIterator`, `Extend`, and a `Deref` to the slice
that keeps the growth API private.

```rust,run
pub struct Bag<T>(Vec<T>);

impl<T> IntoIterator for Bag<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}

impl<'a, T> IntoIterator for &'a Bag<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter { self.0.iter() }
}

impl<T> FromIterator<T> for Bag<T> {
    fn from_iter<I: IntoIterator<Item = T>>(src: I) -> Self { Bag(src.into_iter().collect()) }
}

impl<T> Extend<T> for Bag<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, src: I) { self.0.extend(src) }
}

// Deref to the slice, not to the Vec. The read API is exposed; `push` and `truncate` are not.
impl<T> std::ops::Deref for Bag<T> {
    type Target = [T];
    fn deref(&self) -> &[T] { &self.0 }
}

fn main() {
    let mut b: Bag<u32> = (0..3).collect();
    b.extend([3, 4]);
    assert_eq!(b.len(), 5);                       // through the [T] deref
    assert_eq!(b.iter().copied().max(), Some(4)); // through the [T] deref
    assert_eq!(b.into_iter().count(), 5);         // through the trait impl
}
```

## An exact-size, double-ended iterator

`size_hint` is overridden before `ExactSizeIterator`, and `next` and `next_back` meet in the
middle without handing out one element twice.

```rust,run
pub struct Ids { lo: u32, hi: u32 }

impl Iterator for Ids {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.lo >= self.hi { return None; }
        self.lo += 1;
        Some(self.lo - 1)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.hi.saturating_sub(self.lo) as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Ids {}

impl DoubleEndedIterator for Ids {
    fn next_back(&mut self) -> Option<u32> {
        if self.lo >= self.hi { return None; }
        self.hi -= 1;
        Some(self.hi)
    }
}

fn main() {
    assert_eq!(Ids { lo: 0, hi: 3 }.len(), 3);
    assert_eq!(Ids { lo: 0, hi: 3 }.enumerate().rev().collect::<Vec<_>>(),
               vec![(2, 2), (1, 1), (0, 0)]);
    // next and next_back meet in the middle and hand out each element once.
    let mut ids = Ids { lo: 0, hi: 3 };
    assert_eq!((ids.next(), ids.next_back(), ids.next(), ids.next_back()),
               (Some(0), Some(2), Some(1), None));
}
```

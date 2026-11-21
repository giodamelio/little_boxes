# Iterator diagnostics, verbatim

Deep material for the rust-iterator-impl skill. Every error text below comes from rustc 1.98.1,
edition 2024, on aarch64-apple-darwin.

## A container with only an inherent iter

`for v in &b` on a container that offers only `fn iter(&self)`:

```text
error[E0277]: `&Bag` is not an iterator
5 |     for v in &b { println!("{v}"); }
  |              ^^ `&Bag` is not an iterator
  = help: the trait `Iterator` is not implemented for `&Bag`
  = note: required for `&Bag` to implement `IntoIterator`
```

## An inherent into_iter

`clippy::should_implement_trait` on an inherent `fn into_iter`:

```text
warning: method into_iter can be confused for the standard trait method std::iter::IntoIterator::into_iter
```

## The self.into_iter trap

This body is the trap:

```rust
pub struct Bag { items: Vec<u32> }

impl IntoIterator for Bag {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;
    fn into_iter(self) -> Self::IntoIter { self.into_iter() }   // calls itself
}
```

It compiles and links. The only signal is a warning:

```text
warning: function cannot return without recursing
5 |     fn into_iter(self) -> Self::IntoIter { self.into_iter() }
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^   ---------------- recursive call site
  = note: `#[warn(unconditional_recursion)]` on by default
```

Running it aborts with exit 134:

```text
thread 'main' (<id>) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

## A Deref newtype is not a collection

The failures a `Deref<Target = Vec<T>>` newtype still has, all measured on one wrapper:

| Call site | Result with only `Deref` |
| --- | --- |
| `for v in w` | `error[E0277]: WrappedVec<i32> is not an iterator` |
| `let c: WrappedVec<i32> = (0..3).collect();` | `error[E0277]: a value of type WrappedVec<i32> cannot be built from an iterator over elements of type {integer}` |
| `total(w)`, where `total` takes `I: IntoIterator` | `error[E0277]: WrappedVec<i32> is not an iterator` |
| `w.into_iter()` | `error[E0507]: cannot move out of dereference of WrappedVec<i32>` |
| `w.extend([2, 3])` | `error[E0596]: cannot borrow data in dereference of WrappedVec<i32> as mutable` |

Adding `DerefMut` makes the last row compile, because `extend` is a method call. It fixes none
of the others.

## ExactSizeIterator without a size_hint override

The first `len()` call panics with exit 101:

```text
thread 'main' (<id>) panicked at .../core/src/iter/traits/exact_size.rs:<line>:<col>:
assertion `left == right` failed
  left: None
 right: Some(0)
```

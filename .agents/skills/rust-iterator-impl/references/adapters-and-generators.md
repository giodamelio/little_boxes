# Adapter bounds and stable generators

Deep material for `rust-iterator-impl`. Every error text and number below comes from rustc
1.98.1, edition 2024, on aarch64-apple-darwin.

Contents:

- [Which adapter loses the exact length](#which-adapter-loses-the-exact-length)
- [Three orderings, three results](#three-orderings-three-results)
- [The from_fn shape](#the-from_fn-shape)
- [Old generator snippets](#old-generator-snippets)
- [Two neighbours of from_fn](#two-neighbours-of-from_fn)
- [What from_fn costs](#what-from_fn-costs)

## Which adapter loses the exact length

`Enumerate<I>` implements `DoubleEndedIterator` only when `I: DoubleEndedIterator +
ExactSizeIterator`, because it must know the length to give the correct index from the back.
The error blames `.rev()`:

```text
error[E0277]: the trait bound `Chars<'_>: ExactSizeIterator` is not satisfied
   |     let x: Vec<_> = s.chars().enumerate().rev().collect();
   |                                           ^^^ the trait `ExactSizeIterator`
   |                                               is not implemented for `Chars<'_>`
   = note: required for `Enumerate<Chars<'_>>` to implement `DoubleEndedIterator`
note: required by a bound in `rev`
```

The caret sits on `.rev()`. The adapter that broke the bound is `.chars()`, two positions
earlier. `filter` gives the same error in the same place: `the trait bound
Filter<std::slice::Iter<'_, u32>, {closure@...}>: ExactSizeIterator is not satisfied`. A second
error follows on the next method call: `error[E0599]: the method collect exists for struct
Rev<Enumerate<Chars<'_>>>, but its trait bounds were not satisfied`. Fix the E0277; the E0599
goes with it.

Do not learn this as "`enumerate` breaks `rev`". `v.iter().enumerate().rev()` compiles, because
slice iterators are `ExactSizeIterator`. The adapter between the source and `enumerate` decides:

| Keeps the exact length | Loses it |
| --- | --- |
| `map`, `copied`, `cloned`, `enumerate`, `rev`, `take`, `skip`, `step_by`, `zip`, `peekable` | `filter`, `filter_map`, `flat_map`, `flatten`, `take_while`, `skip_while`, `scan`, `chain` |
| `slice::windows`, `slice::chunks_exact`, `str::bytes` | `str::chars`, `str::split`, `BufRead::lines` |

`chain` is the surprise in that table: two exact lengths can overflow `usize`, so `Chain` is
never `ExactSizeIterator`.

## Three orderings, three results

```rust,run
fn main() {
    let v = ["a", "b", "c"];

    // Keeps the forward index. Needs ExactSizeIterator.
    let keep: Vec<_> = v.iter().enumerate().rev().collect();
    assert_eq!(keep, vec![(2, &"c"), (1, &"b"), (0, &"a")]);

    // Renumbers from 0 at the tail. Compiles on any DoubleEndedIterator.
    let renumber: Vec<_> = v.iter().rev().enumerate().collect();
    assert_eq!(renumber, vec![(0, &"c"), (1, &"b"), (2, &"a")]);

    // `filter` loses the exact length. Collect first, then enumerate.
    let kept: Vec<_> = v.iter().filter(|s| **s != "b").collect();
    let late: Vec<_> = kept.into_iter().enumerate().rev().collect();
    assert_eq!(late, vec![(1, &"c"), (0, &"a")]);
}
```

Both forms yield the same elements in the same order, so a test that checks only the elements
passes with either. The indices are mirrored. Assert on the pair, not on the element.

## The from_fn shape

A `move` closure owns the state, and the function returns `impl Iterator`:

```rust,run
fn evens(limit: u32) -> impl Iterator<Item = u32> {
    let mut next = 0;
    std::iter::from_fn(move || {
        if next >= limit { return None; }
        next += 2;
        Some(next - 2)
    })
}

fn main() {
    assert_eq!(evens(10).collect::<Vec<_>>(), vec![0, 2, 4, 6, 8]);
    assert_eq!(evens(10).size_hint(), (0, None));
}
```

## Old generator snippets

Old snippets add two traps. `std::ops::Generator` does not exist: `use std::ops::Generator;`
gives `error[E0432]: unresolved import std::ops::Generator` on every channel. The trait is now
`std::ops::Coroutine`, with `CoroutineState`, still behind `#![feature(coroutine_trait)]`
(issue 43122). A nightly `gen` block is an `Iterator` only and takes no resume argument, so it
buys no state-carrying routine that stable lacks.

## Two neighbours of from_fn

Both cover a common shape with less code than `std::iter::from_fn`. Use
`std::iter::successors` when each item is a function of the previous one, and
`std::iter::repeat_with` when the closure needs no seed:

```rust,run
fn main() {
    let powers: Vec<u32> = std::iter::successors(Some(1u32), |n| n.checked_mul(2))
        .take(5)
        .collect();
    assert_eq!(powers, vec![1, 2, 4, 8, 16]);

    let mut n = 0;
    let counted: Vec<u32> = std::iter::repeat_with(|| { n += 1; n }).take(3).collect();
    assert_eq!(counted, vec![1, 2, 3]);
}
```

## What from_fn costs

| Property | `FromFn` | Effect |
| --- | --- | --- |
| `size_hint` | always `(0, None)` | `collect` runs the whole growth ladder |
| `DoubleEndedIterator` | never | `.rev()` gives `error[E0277]: the trait bound std::iter::FromFn<{closure@...}>: DoubleEndedIterator is not satisfied` |
| `ExactSizeIterator` | never | `len()` does not exist |
| `Clone` | only when the closure is `Clone` | A closure that captures `&mut` fails with `error[E0277]: the trait bound &mut Vec<u32>: Clone is not satisfied`; a `move` closure over `Copy` state does clone, and the clone forks the sequence: both copies then yield 1, then 2 |

Write a named struct with a manual `Iterator` impl as soon as a caller needs `len()`, `.rev()`,
or an independent `Clone`. The `Ids` struct in
[size_hint is a contract](../SKILL.md#size_hint-is-a-contract) is that form.

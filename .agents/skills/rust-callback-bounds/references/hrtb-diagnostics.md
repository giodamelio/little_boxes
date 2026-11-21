# Higher-ranked callback diagnostics

Worked examples for the rules in [SKILL.md](../SKILL.md). Every quoted diagnostic comes from rustc
1.98.1, edition 2024, on aarch64-apple-darwin. Read only the section that matches the task.

- [A free type parameter cannot name 'a](#a-free-type-parameter-cannot-name-a)
- [A borrow-returning bound in use](#a-borrow-returning-bound-in-use)
- [A fixed 'a permits the escape](#a-fixed-a-permits-the-escape)
- [A let binding loses the higher-ranked expectation](#a-let-binding-loses-the-higher-ranked-expectation)
- [A custom trait supplies no closure-signature expectation](#a-custom-trait-supplies-no-closure-signature-expectation)
- [The hoisted-lifetime dead end](#the-hoisted-lifetime-dead-end)

## A free type parameter cannot name 'a

A generic `K` against `|order| &order.country` fails:

```rust,compile_fail
struct Order { country: String }

fn sort_by_key<T, K: Ord>(_arr: &mut [T], _key: impl FnMut(&T) -> K) {}

fn process(orders: &mut [Order]) {
    sort_by_key(orders, |order| &order.country);
}
```

The diagnostic:

```text
error: lifetime may not live long enough
7 |     sort_by_key(orders, |order| &order.country);
  |                          ------ ^^^^^^^^^^^^^^ returning this value requires that `'1` must outlive `'2`
  |                          |    |
  |                          |    return type of closure is &'2 String
  |                          has type `&'1 Order`
```

`Vec::sort_by_key` reports the same text, word for word.

## A borrow-returning bound in use

Put the output inside the binder. The bound accepts an unannotated closure:

```rust
struct Order { country: String, code: u32 }

fn sort_by_key_ref<T, K: Ord + ?Sized>(
    arr: &mut [T],
    mut key: impl for<'a> FnMut(&'a T) -> &'a K,
) {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if key(&arr[j]) < key(&arr[i]) { arr.swap(i, j); }
        }
    }
}

fn main() {
    let mut v = vec![
        Order { country: "cz".into(), code: 3 },
        Order { country: "at".into(), code: 1 },
    ];
    sort_by_key_ref(&mut v, |o| &o.country); // bare closure, no annotation
    assert_eq!(v[0].code, 1);
}
```

With a second reference argument, write the binder explicitly; elision refuses it with
`error[E0106]`. `for<'a> FnMut(&'a A, &'a B) -> &'a K` compiles and ties both arguments to one
region, so give the second its own `'b` unless you mean to tie them. The shape also works for a
`T` that carries a lifetime, such as `Order<'s>`; a trait with a lifetime parameter can fail
there (see [key-projections.md](key-projections.md#generic-outputs-need-rpitit-or-a-gat)).

## A fixed 'a permits the escape

`for<'a>` on a callback bound forbids the callback to store the reference. A fixed `'a` on the
function silently permits the store:

```rust,compile_fail,E0521
struct Order { code: u32 }

// Fixed 'a: the callback IS allowed to keep the reference.
fn visit_fixed<'a, T, K>(arr: &'a [T], mut key: impl FnMut(&'a T) -> K) {
    for e in arr { key(e); }
}
// HRTB: the callback cannot name any place that outlives the call.
fn visit_hrtb<T, K>(arr: &[T], mut key: impl for<'x> FnMut(&'x T) -> K) {
    for e in arr { key(e); }
}

fn main() {
    let v = vec![Order { code: 7 }];
    let mut stash: Option<&Order> = None;
    visit_fixed(&v, |o| { stash = Some(o); });  // accepted
    assert_eq!(stash.unwrap().code, 7);

    let mut stash2: Option<&Order> = None;
    visit_hrtb(&v, |o| { stash2 = Some(o); });  // error[E0521]
}
```

```text
error[E0521]: borrowed data escapes outside of closure
18 |     let mut stash2: Option<&Order> = None;
   |         ---------- `stash2` declared here, outside of the closure body
19 |     visit_hrtb(&v, |o| { stash2 = Some(o); });
   |                     -    ^^^^^^^^^^^^^^^^ `o` escapes the closure body here
   |                     |
   |                     `o` is a reference that is only valid in the closure body
```

`visit_fixed` compiles because its body does not mutate the collection. That is the silent case:
widen `for<'a>` to a fixed `'a` and the API permits the store from then on.

## A let binding loses the higher-ranked expectation

The same closure text satisfies the bound inline and fails after a `let`:

```rust,compile_fail,E0308
struct Order { country: String }
fn register<F: for<'a> FnMut(&'a Order) -> &'a String>(_f: F) {}

fn main() {
    register(|o: &Order| &o.country);  // OK: the expectation is higher-ranked

    let g = |o: &Order| &o.country;    // error: lifetime may not live long enough
    register(g);                       // error[E0308]: one type is more general
}
```

```text
error: lifetime may not live long enough
7 |     let g = |o: &Order| &o.country;
  |                 -     - ^^^^^^^^^^ returning this value requires that `'1` must outlive `'2`

error[E0308]: mismatched types
8 |     register(g);
  |     ^^^^^^^^^^^ one type is more general than the other
  |
  = note: expected reference `&String`
             found reference `&'a String`
```

A bare `let` supplies no expectation, so each region becomes a fresh inference variable and
resolves to one fixed region. The three repairs are in `SKILL.md`: a `fn(..)` annotation, a
`&dyn for<'a> Fn(..)` annotation, or `hrtb_ref`.

## A custom trait supplies no closure-signature expectation

The bound is higher-ranked and it lives on a user trait, not on `Fn*`, so no closure form is
accepted. A named `fn` item is:

```rust
trait FnOutput<In> { type Output; fn call(&mut self, i: In) -> Self::Output; }
impl<F, In, Out> FnOutput<In> for F where F: FnMut(In) -> Out {
    type Output = Out;
    fn call(&mut self, i: In) -> Out { self(i) }
}
struct Order { country: String }

fn sort_by_key<T, F>(arr: &mut [T], mut key: F)
where
    for<'a> F: FnOutput<&'a T>,
    for<'a> <F as FnOutput<&'a T>>::Output: Ord,
{
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if key.call(&arr[j]) < key.call(&arr[i]) { arr.swap(i, j); }
        }
    }
}

// Accepted: a `fn` item's lifetimes are late bound at declaration, so the
// item type is already higher-ranked.
fn country(x: &Order) -> &str { &x.country }
fn ok(orders: &mut [Order]) { sort_by_key(orders, country); }
```

Three closure forms against that same bound, three failures:

```text
// sort_by_key(orders, |o| &o.country)
error[E0282]: type annotations needed
   |                          ^   - type must be known at this point
   = help: consider giving this closure parameter an explicit type

// sort_by_key(orders, |o: &Order| &o.country)
error: lifetime may not live long enough
   |         -     - ^^^^^^^^^^ returning this value requires that `'1` must outlive `'2`

// sort_by_key(orders, |o: &Order| -> &str { &o.country })
error: lifetime may not live long enough
   |         -          -      ^^^^^^^^^^ returning this value requires that `'1` must outlive `'2`
```

Annotating the return type does not help. The same holds for `|o| async move { .. }` against an
`AsyncFn` bound: E0282 when the body uses `o`, else `implementation of AsyncFn is not general
enough`, and `lifetime may not live long enough` once `|o: &T|` is annotated (Rust 1.99.1). Write
`async |o| ..` there. Wrap the closure in `hrtb_ref(..)` from `SKILL.md`, or
pass a named `fn`.

## The hoisted-lifetime dead end

Start from `fn sort_by_key<'a, T, K: Ord>(arr: &mut [T], mut key: impl FnMut(&'a T) -> K)`.
rustc reports all three errors at once:

```text
error[E0309]: the parameter type `T` may not live long enough
help: consider adding an explicit lifetime bound       // T: 'a

error[E0621]: explicit lifetime required in the type of `arr`
4 |             if key(&arr[j]) < key(&arr[i]) { arr.swap(i, j); }
  |                ^^^^^^^^^^^^ lifetime `'a` required
help: add explicit lifetime `'a` to the type of `arr`  // arr: &'a mut [T]

error[E0502]: cannot borrow `*arr` as mutable because it is also borrowed as immutable
4 |             if key(&arr[j]) < key(&arr[i]) { arr.swap(i, j); }
  |                ------------                  ^^^^^^^^^^^^^^ mutable borrow occurs here
  |                |   |
  |                |   immutable borrow occurs here
  |                argument requires that `arr[_]` is borrowed for `'a`
```

Follow both `help:` lines and this is the terminal state. Only E0502 remains, and no change to the
body can fix it:

```rust,compile_fail,E0502
// Terminal state of the "just add lifetimes" path. E0502, unfixable in the body.
fn sort_by_key<'a, T: 'a, K: Ord>(arr: &'a mut [T], mut key: impl FnMut(&'a T) -> K) {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if key(&arr[j]) < key(&arr[i]) { arr.swap(i, j); }
        }
    }
}
```

A fixed `'a` makes every element handed to the callback stay borrowed for the whole of `'a`, which
outlives the loop body, so the body can never mutate the collection. Restore the elided argument
and pick a row from the decision table in `SKILL.md`.

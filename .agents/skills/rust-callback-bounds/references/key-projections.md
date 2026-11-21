# Key projections and generic callback outputs

Deep material for [SKILL.md](../SKILL.md). Read only the section that matches the task.

- [Change what the callback receives](#change-what-the-callback-receives)
- [Generic outputs need RPITIT or a GAT](#generic-outputs-need-rpitit-or-a-gat)

## Change what the callback receives

Before you build a projection DSL, change what the callback is handed. These both compile with
the very projection `Vec::sort_by_key` rejects:

```rust
struct Order { country: String, code: u32 }

fn main() {
    let v = vec![
        Order { country: "cz".into(), code: 3 },
        Order { country: "at".into(), code: 1 },
    ];
    // v.sort_by_key(|o| &o.country);   // would NOT compile

    let m = v.iter().max_by_key(|o| &o.country).unwrap();
    assert_eq!(m.code, 3);

    let mut refs: Vec<&Order> = v.iter().collect();
    refs.sort_by_key(|o| &o.country);
    assert_eq!(refs[0].code, 1);
}
```

For an iterator of `&'s T` the closure parameter is `&'x &'s T`. `&o.country` reborrows through
the inner `&'s T` and yields `&'s String`, whose lifetime the collection fixes, not the callback
binder. The higher-ranked `'x` never reaches the output, so a fixed `K` is satisfiable.

`Vec::sort_by(|a, b| a.country.cmp(&b.country))` is the third escape and needs no new type at
all. Reach for it before any of the trait machinery below.

## Generic outputs need RPITIT or a GAT

A composite your signature can name still fits a higher-ranked `Fn` bound.
`impl for<'a> FnMut(&'a T) -> Reverse<&'a str>` and `impl for<'a> FnMut(&'a T) -> (&'a str, u32)`
both accept a bare closure. Reach for a trait only when the output must stay a type *parameter*,
because no `Fn` bound can write `-> K<'a>`. `impl FnMut(&T) -> impl Ord` is rejected:
``error[E0562]: `impl Trait` is not allowed in the return type of `Fn` trait bounds``. Declare a
one-method trait with `fn project<'a>(&mut self, i: &'a In) -> impl Ord + use<'a, Self, In>`, and
have callers pass a named unit struct instead of a closure. That loss of the closure is the cost
of this row. `use<..>` in a trait method needs Rust 1.87, and it is load-bearing: without it the
result also captures the `&mut self` borrow, and two calls in one comparison fail with E0499.

Read [storing-callables.md](storing-callables.md#composite-outputs-rpitit) when you
write this trait. It has the runnable examples and three traps: repeat `impl Ord + use<..>` in
the impl (a concrete type triggers `refining_impl_trait`), keep the lifetime off the trait
(`for<'a> K: KeyProjection<'a, T>` breaks on a borrowed `T`), and use a GAT `type Out<'a>` when
callers must name the output.

# Coroutines and generators over `&mut State`

Evidence for `suspendable-routines.md`. Read it when someone proposes an
`async fn(&mut State)`, a waker or context side channel, nightly coroutines, a stable
generator crate, `std::ops::Generator`, or a `gen` block as the way to give a suspendable
routine `&mut State`.

- [`Future` has no slot for the state](#future-has-no-slot-for-the-state)
- [The three escapes, and why none works](#the-three-escapes-and-why-none-works)
- [The resume argument is not higher-ranked](#the-resume-argument-is-not-higher-ranked)
- [Stable generator crates have the same limit](#stable-generator-crates-have-the-same-limit)
- [Two stale spellings](#two-stale-spellings)

The stable diagnostics come from rustc 1.99.1, edition 2024. The coroutine diagnostics come
from nightly rustc 1.97.0-nightly (d7f14d3d8 2026-05-15). Coroutines, the `Coroutine` trait,
and `gen` blocks are nightly-only as of Rust 1.99.1.

## `Future` has no slot for the state

`Future::poll` takes `&mut Context<'_>` and nothing else. It has no slot for a
caller-supplied `&mut State`. An `async fn(&mut State)` captures the borrow at construction
and holds it for the whole life of the future, so two such routines over one state conflict
before any executor is involved:

```rust,compile_fail,E0499
struct State { counter: u32 }

async fn routine(st: &mut State) { st.counter += 1; }

async fn run(st: &mut State) {
    let a = routine(st);
    let b = routine(st);
    a.await;
    b.await;
}
```

```text
error[E0499]: cannot borrow `*st` as mutable more than once at a time
6 |     let a = routine(st);
  |                     -- first mutable borrow occurs here
7 |     let b = routine(st);
  |                     ^^ second mutable borrow occurs here
8 |     a.await;
  |     - first borrow later used here
```

## The three escapes, and why none works

| Escape | Verdict |
| --- | --- |
| Smuggle `*mut State` through `Waker::data()` | The pointer does not survive `FuturesUnordered`, or anything built on it, which substitutes its own per-future waker. Two `&mut State` made from it are undefined behavior under both Miri borrow models |
| `Context::ext()` (nightly) | Carries a value only if the *executor* built the context with `ContextBuilder::ext()`. tokio uses `Context::from_waker`, so `ext()` is `()`, and every downcast returns `None` with no diagnostic |
| Nightly coroutines with a resume argument | A `#[coroutine]` closure implements `Coroutine<&'x mut State>` for **one** inferred lifetime. The higher-ranked request fails with `implementation of Coroutine is not general enough`, and the one-lifetime version fails with `E0499` at the second `resume` in the loop. Stabilization would not change this |

The missing feature is a higher-ranked binder over the resume type, not stabilization. The
next two sections hold the evidence.

## The resume argument is not higher-ranked

A `#[coroutine]` closure implements `Coroutine<&'x mut State>` for **one** inferred
lifetime. The loop needs a fresh `&mut State` on every `resume`, so it needs the bound for
every lifetime. Asking for the higher-ranked bound fails:

```rust,ignore
#![feature(coroutine_trait, coroutines, stmt_expr_attributes)]
use std::ops::Coroutine;
struct State { counter: u32 }

fn routine() -> impl for<'x> Coroutine<&'x mut State, Yield = (), Return = ()> {
    #[coroutine] |mut st: &mut State| {
        st.counter += 1;
        st = yield ();
        st.counter += 10;
    }
}
```

```text
error: implementation of `Coroutine` is not general enough
 6 |       #[coroutine] |mut st: &mut State| {
   |  __________________^
   ...
10 | |     }
   | |_____^ implementation of `Coroutine` is not general enough
   = note: `{coroutine@src/main.rs:6:18: 6:38}` must implement `Coroutine<&'1 mut State>`,
     for any lifetime `'1`...
   = note: ...but it actually implements `Coroutine<&'2 mut State>`, for some specific
     lifetime `'2`
```

Accepting one lifetime, `fn routine<'x>() -> impl Coroutine<&'x mut State, ...>`, compiles
the definition. It then fails at the only call site that matters, the loop:

```text
error[E0499]: cannot borrow `st` as mutable more than once at a time
18 |         match c.as_mut().resume(&mut st) {
   |               -                 ^^^^^^^ `st` was mutably borrowed here in the previous
   |                                         iteration of the loop
   |               first borrow used here, in later iteration of loop
```

The missing feature is a higher-ranked binder over the resume type. Stabilization of
coroutines does not add it. `for<...> impl Trait` is not a valid spelling either. On stable
1.99.1 it gives ``error: `for<...>` expected after `impl`, not before`` with
``help: move `impl` before the `for<...>` ``.

## Stable generator crates have the same limit

The stable generator crates wrap an `async` block and expose a `resume(value)` API. They fix
the resume type once, when the generator is built. Measured with genawaiter 0.99.1 on rustc
1.98.1:

- A higher-ranked request, `impl for<'a> Coroutine<Yield = (), Resume = &'a mut State, ...>`,
  fails with `E0582`: the binding for `Resume` references `'a`, which does not appear in the
  trait input types.
- The one-lifetime version compiles. A loop that calls `resume_with(&mut st)` twice then
  fails with the same `E0499` as above, "mutably borrowed here in the previous iteration of
  the loop".

Those crates are useful for yielded values. They are not a route to `&mut State`.

## Two stale spellings

The `rust-iterator-impl` skill, when it is installed, owns these facts and the stable
`iter::from_fn` route for a routine that only yields values.

- `std::ops::Generator` does not exist. `use std::ops::Generator;` is `E0432`. The nightly
  trait is `std::ops::Coroutine`.
- A `gen { .. }` block (nightly) is an `Iterator` only. It has no resume argument.

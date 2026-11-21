---
description: Use when designing or reviewing an event loop, tick loop, or handler registry whose handlers all need &mut to one shared mutable state, such as a game loop, a TUI loop, or a god object. Also for E0499 in a dispatch loop, E0502 on a queue peek, E0207 or E0119 on a generic Handler trait, Rc RefCell between handlers, an ECS system and world, and coroutine resume or async fn as a suspendable routine over that state. Not for async runtime scheduling; use rust-async-internals.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-event-loop-state
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: d752ff7f0cfb3c65b3c9d83b35fc625d870a5ba9
name: rust-event-loop-state
---
# Rust event loop state

**The loop owns the handler set and the state as two separate values, and passes one into
the other.** Every rule below follows from this sentence. `E0499` in a dispatch loop almost
never needs a different line. It needs a different owner.

This skill covers synchronous loops; under tokio, use the `rust-async-internals` skill when it
is installed. The diagnostics below are from rustc 1.99.1, edition 2024.

## Pick the structure from the shape of the handler set

Stop at the first row that describes your handler set. Each row down has a concrete cost.

| Shape of the handler set | Structure | What it costs |
| --- | --- | --- |
| Fixed at compile time. The state's fields are known to the crate that writes the loop | The loop holds `Vec<Box<dyn Handler>>` and `State` as two locals. Dispatch `h.handle(&mut state, &ev)` | Nothing. Start here |
| Handlers must be reused across applications that have different states | State is a trait parameter: `trait Handler<S>`. Each handler declares the capability traits it needs on `S`. The loop registers `Vec<Box<dyn Handler<App>>>` | Trait-solver error volume, and a blanket impl forecloses a later per-state impl on that type for every state that meets its bounds (`E0119`) |
| The component set is open at run time: plugins, scripting, save files, or a level editor decide it | An ECS-shaped dynamic world with type-erased components and systems | Aliasing detection stops being a compile error. It becomes a run-time panic the first time the schedule initializes that system |
| None of the above | Row 1. Do not reach further | Nothing. The shortcuts in [Do not do this](#do-not-do-this) each move a compile error to a panic, a deadlock, or an unfixable `E0499` |

## Triage

| Symptom | Cause | Fix | Section |
| --- | --- | --- | --- |
| `E0499: cannot borrow *st as mutable more than once`, first borrow at `iter_mut()`, second at the dispatch argument | The handler collection is a field of the state | Move the field out of `State`, dispatch a sub-struct field, or `mem::take` it for the tick | [The loop owns both](#the-loop-owns-both-separately) |
| `E0502 ... immutable borrow later used by call` on `while let Some(ev) = st.queue.front()` | The event is borrowed out of a queue inside the state | Pop the event by value | [Drain the queue](#drain-the-queue-by-value) |
| `E0499 ... first borrow later used by call`, first borrow at handler construction | A handler holds a `&mut` into the state | Store an index or a generational key | [Store a key](#a-handler-stores-a-key-never-a-borrow) |
| `E0207: the type parameter S is not constrained` | The handler trait has `type State` | Make it `trait Handler<S>` | [State is a trait parameter](references/state-partitioning.md#state-is-a-trait-parameter-not-an-associated-type) |
| `E0119: conflicting implementations of trait Handler<App>` | A blanket `impl<S: ...> Handler<S> for X` whose bounds `App` satisfies. An unbounded one covers every state | Keep one impl: move per-state behavior behind a capability-trait method, or replace the blanket impl with per-state impls | [Blanket impl](references/state-partitioning.md#the-blanket-impl-and-e0119) |
| `E0499` on `&mut ctx.field` where `ctx` implements `DerefMut` | Auto-deref borrows the whole wrapper | Plain fields, or one `split()` method | [No DerefMut](references/state-partitioning.md#static-plus-dynamic-state-three-versions) |
| Run-time panic `... conflicts with a previous system parameter` (an ECS code such as `B0001`) | Two overlapping mutable views in one system | A parameter set or a disjoint filter | [ECS](references/state-partitioning.md#ecs-trade) |
| `RefCell already borrowed` or `RefCell already mutably borrowed` panic, or a hang on a nested `lock()` | Shared state behind `Rc<RefCell<_>>` or `Arc<Mutex<_>>`, re-entered during dispatch | Row 1 of the structure table | [Do not do this](#do-not-do-this) |
| A routine must pause mid-work and continue next tick | - | A `resume(&mut self, &mut State)` trait | [Suspendable routines](references/suspendable-routines.md#suspendable-routines-over-shared-state) |
| `implementation of Coroutine is not general enough` | A coroutine resume argument is not higher-ranked | Same as the row above | [Suspendable routines](references/suspendable-routines.md#suspendable-routines-over-shared-state) |

## Do not do this

| Move | What actually happens |
| --- | --- |
| `Rc<RefCell<State>>` cloned into every handler, re-borrowed during dispatch | The `E0499` becomes a `RefCell already borrowed` panic the first time one handler calls another |
| `Arc<Mutex<State>>` inside one thread, re-locked during dispatch | The nested `lock()` deadlocks or panics (std leaves it unspecified), with no output and no load needed |
| `unsafe { &mut *ptr }` to hand two handlers the state | Two live `&mut` to one place are undefined behavior. Miri rejects it under Stacked Borrows and Tree Borrows |
| Two live `async fn(&mut State)` routines over one state | `E0499` before any executor runs: each future holds the `&mut` until it completes |
| An ECS taken for ergonomics, not for an open component set | Run-time conflict panics for a component set the compiler could check |
| `std::mem::take(&mut st.handlers)`, then `st.handlers = hs` | Every handler registered during the tick is silently dropped. Call `hs.append(&mut st.handlers)` first |
| A bare `usize` slot key when slots are removed or reused | The key names a different slot, or an out-of-bounds slot, with no error. Use a generational key |

The first two rows are about one synchronous loop: `Arc<Mutex<State>>` across separate tasks is
a valid choice.

## Verify

The change is complete when every row that applies to it is green.

| Claim | Check | What a green result does not prove |
| --- | --- | --- |
| The loop, handlers, and state borrow correctly (structure rows 1 and 2) | `cargo check --locked --all-targets` on the crate | Nothing about a `RefCell` or `Mutex` that remains, or about ECS access |
| No re-entrant `RefCell` or `Mutex` path remains | `rg -e 'Rc<RefCell<' -e 'Arc<Mutex<'` over the state and handler types. Then a test that dispatches one event through every handler, including a handler that triggers another, under the timeout below | Paths the test does not reach. A cell behind a type alias |
| A handler that enqueues cannot stall the tick | A test with a handler that enqueues on every event. Run one tick under the timeout below | Handlers that enqueue only on some events |
| No ECS access conflict | A test that initializes and runs every real schedule once against an empty world | Systems added to a schedule at run time |
| Dispatch that still contains `unsafe` does not alias | `cargo +nightly miri test --locked` on the dispatch tests when nightly `miri` is installed; otherwise report it as not checked by Miri | Paths the tests do not reach |

The `rust-sanitizers-miri` skill, when it is installed, owns the Miri flags. Without it, gate on
the default Stacked Borrows run and treat `MIRIFLAGS="-Zmiri-tree-borrows"` as a second opinion.

A re-locked `Mutex` and an always-enqueuing `while let` drain usually fail as a hang, and
libtest never ends a hung test. Run one tick inside `std::thread::spawn` and assert
`rx.recv_timeout(Duration::from_secs(5)).is_ok()` on a channel that the thread signals. Read
`references/review-checklist.md` ("Hang test") for the harness, and for the review questions
when you review an existing loop.

## The loop owns both, separately

Put the handler collection inside the state and dispatch `&mut State` into the handlers, and
the call cannot type-check. The iterator holds a mutable borrow of `*st` for the whole loop
body, so the `&mut State` argument is a second mutable borrow of the same place:

```rust,compile_fail,E0499
struct Event(u32);
struct State { counter: u32, handlers: Vec<Box<dyn Handler>> }
trait Handler { fn handle(&mut self, st: &mut State, ev: &Event); }

fn tick(st: &mut State, ev: Event) {
    for h in st.handlers.iter_mut() {
        h.handle(st, &ev);
    }
}
```

No interior mutability fixes this. Move the collection out: two locals in the loop are two
owners, and their borrows are disjoint. The "Two owners" section of
`references/state-partitioning.md` has the runnable version.


When an existing codebase cannot move the field yet, use one of two repairs:

- Group the rest of the state in a sub-struct, make the handler trait take that sub-struct,
  and dispatch `&mut st.rest`. `st.handlers` and `st.rest` are disjoint field borrows, so the
  handler field can stay.
- Use `std::mem::take(&mut st.handlers)` for the tick only when a handler must also mutate the
  handler set, and append the newcomers before the put-back. Read the "Migration" section of `references/state-partitioning.md` for the runnable
  version and its panic limit.

## Drain the queue by value

`front()` returns a reference tied to `*st`. The reference is an argument of the dispatch
call, so NLL keeps it live across the call, and the whole state stays borrowed (`E0502`).
`pop_front()` produces an owned `Event`, so no borrow of the state survives into the
handler, and a handler can push more events back:
`while let Some(ev) = st.queue.pop_front() { handle(&mut st, ev); }`. The "Queue drain
diagnostic" section of `references/state-partitioning.md` has the runnable version.


A handler that enqueues on every event makes that `while let` spin forever. Drain one batch
per tick: `for _ in 0..st.queue.len() { let Some(ev) =
st.queue.pop_front() else { break }; handle(st, ev); }`.

## A handler stores a key, never a borrow

A handler built from a field of the state carries that borrow into every dispatch:
`Caching { cache: &mut st.caches[0] }` followed by `h.handle(st)` is `E0499`. Store the slot
instead, `struct Caching { slot: usize }`, and resolve it at the top of each call:
`st.caches[self.slot].hits += 1`. For two slots at once, use
`st.caches.get_disjoint_mut([a, b])` (Rust 1.86+), which returns an `Err` on overlap. A bare
`usize` goes stale with no error when a slot is reused; use a generational key when slots are
freed. Read the "Key-holding handler" section of
`references/state-partitioning.md` for the diagnostic, the runnable version, and the
staleness rules.

## Generic state, dynamic worlds, and suspendable routines

Read `references/state-partitioning.md` when handlers are reused across states (structure row 2), a context pairs a static state with a dynamic world, or the component set is open at run time (structure row 3).

Read `references/suspendable-routines.md` when a routine must pause and resume next tick, and `references/coroutines-and-generators.md` when someone proposes `async fn(&mut State)`, a coroutine, or a generator for it.

## Related skills

When installed: `rust-async-internals` (everything under a runtime), `rust-compiler-errors`
(`E0499` and `E0502` in general), `rust-discipline` (`Deref` on non-pointer types, blanket
impls), `rust-unsafe` (the aliasing rules behind the raw-pointer rows), `rust-iterator-impl`
(`iter::from_fn` for a routine that only yields), and `rust-crate-architecture` (which crate holds
the handler trait; `cargo tree --locked -p <crate>`).

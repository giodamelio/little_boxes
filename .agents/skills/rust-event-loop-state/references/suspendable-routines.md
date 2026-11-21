# Suspendable routines over shared state

Detail for a routine in `SKILL.md` that must pause mid-work and continue next tick.

A routine that must pause mid-work and continue next tick does not need `async`. Give it a
trait whose step method takes the state. Every call takes a fresh reborrow by construction,
which is the property the loop needs:

```rust,run
struct State { counter: u32 }
enum Step { Pending, Done }
trait Routine { fn resume(&mut self, st: &mut State) -> Step; }

struct Counting { left: u32 }
impl Routine for Counting {
    fn resume(&mut self, st: &mut State) -> Step {
        if self.left == 0 { return Step::Done; }
        self.left -= 1;
        st.counter += 1;
        Step::Pending
    }
}

fn main() {
    let mut st = State { counter: 0 };
    let mut routines: Vec<Box<dyn Routine>> = vec![Box::new(Counting { left: 5 })];
    while !routines.is_empty() {
        routines.retain_mut(|routine| {
            matches!(routine.resume(&mut st), Step::Pending)
        });
    }
    assert_eq!(st.counter, 5);
    println!("counter={}", st.counter);   // counter=5
}
```

The cost: you write the state machine by hand, as fields on the routine struct. No `.await`
sugar exists for this shape on stable or nightly.

`Future::poll` takes `&mut Context<'_>` and nothing else, so it has no slot for a
caller-supplied `&mut State`. An `async fn(&mut State)` holds the borrow for the whole life
of the future. Use `async` only for routines that own everything they touch. When two futures
must touch one struct, hand each a `&mut` to a **different field** before the futures are
created. The `rust-async-internals` skill owns that case.

No escape works: not a `*mut State` in `Waker::data()`, not `Context::ext()`, and not a nightly
coroutine. Read `references/coroutines-and-generators.md` when someone proposes one of these
escapes, a stable generator crate, `std::ops::Generator`, or a `gen` block as the fix. It holds
the verdict and the evidence for each.

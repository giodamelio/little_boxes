# Borrow checker fix catalogue

Worked fixes for the errors in [SKILL.md](../SKILL.md). Every fix compiles on rustc 1.98.1.
Each entry states the fix and what it costs, because several of these trade a build error for a
run-time cost. Read only the section that matches the error.

- [Who owns the value](#who-owns-the-value): E0382 instead of a clone
- [Disjoint access to one collection](#disjoint-access-to-one-collection): E0499 on two indices
- [A read borrow that blocks a write](#a-read-borrow-that-blocks-a-write): E0502
- [A `&self` helper that blocks a field write](#a-self-helper-that-blocks-a-field-write): E0506
- [E0507: an owned value out of a reference](#e0507-an-owned-value-out-of-a-reference)
- [E0716: a temporary ended before its borrow](#e0716-a-temporary-ended-before-its-borrow)
- [A borrow that must cross a thread](#a-borrow-that-must-cross-a-thread): E0373, E0521
- [Interior mutability, and its cost](#interior-mutability-and-its-cost)
- [E0106 and lifetime annotation shapes](#e0106-and-lifetime-annotation-shapes)
- [Drop and the borrow checker](#drop-and-the-borrow-checker): E0597, E0502, E0507, E0509,
  E0184, E0367
- [E0631: a function item is not a coercion site](#e0631-a-function-item-is-not-a-coercion-site)

## Who owns the value

A clone answers "both get one". Pick the answer from the kind of value:

| The value is | The answer |
| --- | --- |
| Small, and the copy is the point | Clone, or derive `Copy` |
| Read by several places, never written | `&T`, or `Arc<T>` across a thread |
| Written by several places | `Arc<Mutex<T>>`; check the lock order |
| An identity: a connection, a file, a job | One owner; pass `&mut` or a handle |
| Large and consumed once | Move it; restructure the caller |

## Disjoint access to one collection

The compiler tracks borrows per place, not per element. It cannot prove `v[0]` and `v[1]` are
different places, so two `&mut` into one `Vec` are rejected even when the indices differ.

```rust,compile_fail,E0499
fn bump(v: &mut Vec<i32>) {
    let a = &mut v[0];
    let b = &mut v[1];
    *a += *b;
}
```

```rust
// Split at an index. The two halves are disjoint by construction.
pub fn add_first_to_second(v: &mut [i32]) {
    let (left, right) = v.split_at_mut(1);
    left[0] += right[0];
}

// Several disjoint indices at once. Returns Err(GetDisjointMutError) on a
// duplicate or out-of-range index.
pub fn swap_three(v: &mut [i32]) -> Option<()> {
    let [a, b, c] = v.get_disjoint_mut([0, 2, 4]).ok()?;
    *a += *b + *c;
    Some(())
}

// Iterate and mutate every element. No index, no bounds check.
pub fn double_all(v: &mut [i32]) {
    for slot in v.iter_mut() {
        *slot *= 2;
    }
}

// Two named fields of one struct. Field borrows are already disjoint.
pub struct Pair {
    pub left: Vec<i32>,
    pub right: Vec<i32>,
}

pub fn drain_into(pair: &mut Pair) {
    // Borrowing two distinct fields of the same struct is allowed.
    pair.left.append(&mut pair.right);
}
```

`get_disjoint_mut` is stable since Rust 1.86. Before that the same job needed `split_at_mut`
twice, or an index-and-copy pass.

## A read borrow that blocks a write

```rust,compile_fail,E0502
fn push_first(v: &mut Vec<i32>) {
    let first = &v[0];
    v.push(1);
    println!("{first}");
}
```

```rust
// Fix 1: copy the value out. The borrow ends at the semicolon.
pub fn copy_out(v: &mut Vec<i32>) {
    let first = v[0];
    v.push(first);
}

// Fix 2: scope the borrow, when the value is not Copy.
pub fn scoped(v: &mut Vec<String>) {
    let first = {
        let borrowed = &v[0];
        borrowed.len()
    };
    v.push(first.to_string());
}

// Fix 3: collect the decisions first, then apply them.
pub fn retain_matching(v: &mut Vec<String>, needle: &str) {
    let keep: Vec<bool> = v.iter().map(|s| s.contains(needle)).collect();
    let mut index = 0;
    v.retain(|_| {
        let decision = keep[index];
        index += 1;
        decision
    });
}
```

Fix 3 has a name: compute the plan under a shared borrow, then execute it under an exclusive
borrow. It is the general answer whenever the read informs the write. `retain` alone is shorter
when the predicate needs no outside state.

## A `&self` helper that blocks a field write

A `&self` helper borrows all of `self`. When its result stays live, a write to any field is E0506:

```rust,compile_fail,E0506
pub struct Cache {
    entries: Vec<String>,
    hits: usize,
}

impl Cache {
    pub fn get(&mut self, key: &str) -> Option<&String> {
        let found = self.lookup(key)?;
        self.hits += 1; // cannot assign to `self.hits` because it is borrowed
        Some(found)
    }

    fn lookup(&self, key: &str) -> Option<&String> {
        self.entries.iter().find(|e| *e == key)
    }
}
```

Work on the fields, not on `self`. Borrows of two distinct fields are disjoint:

```rust
pub struct Cache {
    entries: Vec<String>,
    hits: usize,
}

impl Cache {
    pub fn get(&mut self, key: &str) -> Option<&String> {
        let found = self.entries.iter().find(|e| *e == key)?;
        self.hits += 1;
        Some(found)
    }
}
```

A private helper that takes `&self` forces a whole-struct borrow. Free functions that take the
fields, or code written against the fields directly, keep the borrows apart. This is why a large
struct with many `&mut self` methods eventually fights the borrow checker: every method borrows
everything.

## E0507: an owned value out of a reference

```rust,compile_fail,E0507
struct S { name: String }
fn f(s: &S) -> String { s.name }
```

Pick by what should happen to the original:

| Intent | Call |
| --- | --- |
| The original keeps its value | `s.name.clone()` |
| The original is left empty and is still valid | `std::mem::take(&mut s.name)` |
| The original is left holding something else | `std::mem::replace(&mut s.name, other)` |
| The field is optional and becomes `None` | `s.name.take()` on an `Option` |
| The caller is finished with the whole value | Change the signature to take `self` |

`mem::take` needs `&mut` and `Default`. It is the cheapest of these: no allocation, no clone.

```rust
#[derive(Default)]
pub struct Job {
    pub name: String,
    pub payload: Option<Vec<u8>>,
}

// Leaves an empty String behind. No allocation, no clone.
pub fn take_name(job: &mut Job) -> String {
    std::mem::take(&mut job.name)
}

// Leaves a chosen value behind, and returns the old one.
pub fn rename(job: &mut Job, next: String) -> String {
    std::mem::replace(&mut job.name, next)
}

// The Option case. Leaves None behind.
pub fn take_payload(job: &mut Job) -> Option<Vec<u8>> {
    job.payload.take()
}

// Swap two places without a temporary owner.
pub fn swap(a: &mut Job, b: &mut Job) {
    std::mem::swap(a, b);
}
```

`mem::replace` does not require `Default`, which is why it works for a type with no sensible
empty value.

## E0716: a temporary ended before its borrow

```rust,compile_fail,E0716
fn foo() -> Vec<u8> { vec![1, 2, 3] }
fn bar(v: &Vec<u8>) -> &u8 { &v[0] }

// `foo()` produced a temporary with no name. It dies at the end of the statement.
let p = bar(&foo());
let q = *p;
```

Give the temporary a name. That extends it to the end of the enclosing block:

```rust
fn foo() -> Vec<u8> { vec![1, 2, 3] }
fn bar(v: &Vec<u8>) -> &u8 { &v[0] }

let tmp = foo();
let p = bar(&tmp);
let q = *p;
```

Not every temporary dies at the semicolon. `let r = &make();` extends the temporary, and so does
`let r = Some(&make());` since Rust 1.89; on an older MSRV the constructor form is E0716. A
function argument such as `bar(&foo())` is never extended. The `rust-borrow-semantics` skill, when
it is installed, has the exact extension rules, the edition 2024 scope changes, and two-phase
borrows.

## A borrow that must cross a thread

`thread::spawn` requires `'static`, so no borrow of a local can enter it.

```rust
use std::sync::Arc;

// Rejected: the closure outlives the borrow.
// pub fn f(data: &Vec<i32>) {
//     std::thread::spawn(move || println!("{data:?}"));
// }

// Fix 1: share ownership. One allocation, cheap clones.
pub fn shared(data: Arc<Vec<i32>>) {
    let handle = Arc::clone(&data);
    std::thread::spawn(move || println!("{handle:?}"));
}

// Fix 2: a scoped thread. The borrow is allowed because the scope joins
// every thread before it returns.
pub fn scoped(data: &Vec<i32>) {
    std::thread::scope(|s| {
        s.spawn(|| println!("{data:?}"));
    });
}
```

`thread::scope` is stable since Rust 1.63. Prefer it when the work is bounded and the caller can
wait. Use `Arc` when the thread must outlive the calling frame.

A borrowed parameter that reaches `spawn` is E0521. A local that the closure borrows without
`move` is E0373, "closure may outlive the current function, but it borrows `v`". Add `move` when
the thread may own the value. Clone an `Arc` into the closure first when the caller still needs
it.

## Interior mutability, and its cost

Reach for these only after the splits above fail.

| Type | Check | Failure mode | Use when |
| --- | --- | --- | --- |
| `Cell<T>` | none; `get` needs `T: Copy`, while `set`, `replace`, and `take` (`T: Default`) work for any `T` | none | A small field that is replaced whole, never borrowed, single thread |
| `RefCell<T>` | run time | panics on conflict | A graph shape the compiler cannot verify, single thread |
| `Mutex<T>` | run time | blocks, or deadlocks | Shared write access across threads |
| `RwLock<T>` | run time | writer starvation | Many readers, rare writers |
| `AtomicUsize` and friends | none | none | A counter or a flag |

```rust
use std::cell::RefCell;

pub struct Graph {
    nodes: Vec<RefCell<Node>>,
}

pub struct Node {
    pub visited: bool,
}

impl Graph {
    // The borrow lives only inside this call, so the panic window is small.
    pub fn mark(&self, index: usize) {
        self.nodes[index].borrow_mut().visited = true;
    }
}
```

Keep every `borrow_mut` short and never hold one across a call that might re-enter. A `RefCell`
panic reports `RefCell already borrowed` from `borrow_mut`, or `RefCell already mutably borrowed`
from `borrow`. The panic location names the failing call, and `RUST_BACKTRACE=1` shows the path
to it. Nothing records the other live borrow, so shorten every `borrow` and `borrow_mut` guard that
can be live at that call.

For the thread-safe types, see the `memory-model` skill for ordering and the
`rust-async-internals` skill for holding a guard across an `.await`.

## E0106 and lifetime annotation shapes

```rust,compile_fail,E0106
struct S { name: &str }
```

Two answers, and the right one is usually the second:

```rust
struct Borrowed<'a> { name: &'a str }   // the struct cannot outlive the source
struct Owned { name: String }           // the struct owns its data
```

Store owned data unless the type is a short-lived view built inside one function and consumed
inside it. A lifetime parameter on a struct spreads: every type that holds it needs one too, and
the annotation reaches the whole call graph. Pay that cost for a parser view or a zero-copy
frame, not for a config or a message.

When a function returns a reference, state its source with the narrowest lifetime that is true:

```rust
// One input, one output. The lifetime is inferred; do not write it.
pub fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

// Two inputs, one output. The compiler cannot guess, so state the source.
pub fn longer<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

// The output borrows from one input only. Say which, and free the other.
pub fn prefix_of<'a>(text: &'a str, _sep: &str) -> &'a str {
    text.split('=').next().unwrap_or(text)
}

// A struct that borrows. It cannot outlive `source`.
pub struct View<'a> {
    pub source: &'a [u8],
}

impl<'a> View<'a> {
    pub fn head(&self) -> &'a [u8] {
        &self.source[..self.source.len().min(4)]
    }
}
```

The third shape matters: tying the output to both inputs when only one is the source forces the
caller to keep the other alive for no reason. Write the narrowest lifetime that is true.

## Drop and the borrow checker

Adding `impl Drop` to a type is a change to its borrow rules. Several errors follow, and no error
title names `Drop`.

| You add `Drop` to | The new error | Cause |
| --- | --- | --- |
| a type with a lifetime parameter | E0597 on the borrowed local | dropck extends the borrow to the drop point |
| a guard that holds `&mut T` | E0502 at the next read of `T` | the drop point is one more use, after the last visible use |
| any type | E0509 at each partial move out of it | drop glue needs the whole value |
| a type that derives `Copy` | E0184 at the derive | `Copy` and `Drop` are exclusive |

NLL ends a borrow at its last use, and a `Drop` impl adds one last use at the end of the scope.

### E0597: dropck extends the borrow to the drop point

Without a `Drop` impl the compiler knows destruction cannot read `'a`. With one, `drop(&mut self)`
could read the reference, so the borrow must last until the value is dropped. Locals drop in
reverse declaration order, so a guard declared before its source now fails.

```rust,compile_fail,E0597
struct NoDrop<'a>(&'a i32);
struct WithDrop<'a>(&'a i32);
impl Drop for WithDrop<'_> {
    fn drop(&mut self) {}
}

fn ok() {
    let d;
    let x = 5;
    d = NoDrop(&x);      // compiles
    let _ = d.0;
}

fn bad() {
    let d;
    let x = 5;
    d = WithDrop(&x);    // E0597: `x` does not live long enough
    let _ = d.0;
}
```

The note reads "borrow might be used here, when `d` is dropped and runs the `Drop` code for type
`WithDrop`", followed by "values in a scope are dropped in the opposite order they are defined".
Declare the borrowed local before the guard, or keep the type `Drop`-free.

### E0502: the drop point is one more use

```rust,compile_fail,E0502
struct Guard<'a>(&'a mut u32);
impl Drop for Guard<'_> {
    fn drop(&mut self) {}
}

fn read_while_guarded() {
    let mut x = 0u32;
    let _g = Guard(&mut x);
    println!("{x}"); // cannot borrow `x` as immutable because it is also borrowed as mutable
}
```

The note reads "mutable borrow might be used here, when `_g` is dropped and runs the `Drop` code
for type `Guard`". Call `drop(_g)` before the read, or scope the guard in an inner block.

### E0507: `drop` holds `&mut self`, so a field pattern moves

`if let Some(h) = self.0` inside `drop` fails with "cannot move out of `self` as enum variant
`Some` which is behind a mutable reference" for every payload that is not `Copy`. rustc suggests
`if let Some(ref h) = self.0`, which gives a `&Handle` when you usually want `&mut`. Match on
`self` instead: `self` is already a reference, so default binding modes make every binding a
reference.

```rust
pub struct Handle;
impl Handle {
    pub fn report(&self, _reason: &str) {}
}

pub struct Guard(Option<Handle>);

impl Drop for Guard {
    fn drop(&mut self) {
        // `if let Some(h) = self.0` is E0507. Match on `self`; `h` binds as `&mut Handle`.
        let Self(Some(h)) = self else { return };
        h.report("unused");
    }
}
```

Take ownership with `self.0.take()` or `std::mem::replace` when the cleanup must consume the
payload.

### E0509: a `Drop` impl blocks every partial move

```rust,compile_fail,E0509
struct Inner(String);
struct Outer {
    inner: Inner,
}
impl Drop for Outer {
    fn drop(&mut self) {}
}

// E0509: cannot move out of type `Outer`, which implements the `Drop` trait
fn take(o: Outer) -> Inner {
    o.inner
}
```

Drop glue runs on the whole value, so it cannot run on a value with a hole in it. Move `Drop` to a
one-field guard type and keep the aggregate `Drop`-free. The aggregate then allows partial moves,
and the guard still runs its cleanup:

```rust
pub struct Inner(pub String);

pub struct Ticket;
impl Ticket {
    pub fn release(&self) {}
}

// The guard holds only what the cleanup needs.
pub struct ReleaseGuard(pub Ticket);
impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub struct Outer {
    pub inner: Inner,
    pub release: ReleaseGuard,
}

// Compiles: `Outer` has no `Drop` impl. `o.release` still drops at the end of `take`.
pub fn take(o: Outer) -> Inner {
    o.inner
}
```

When the cleanup must consume the guard's field, make that one field an `Option` and `.take()` it
in `drop`, or `mem::replace` it. Do not make the aggregate's fields `Option` instead: every method
then needs an `unwrap`. A consuming method such as `fn into_inner(self) -> Inner { self.inner }`
on a `Drop` type is E0509 as well; write `std::mem::take(&mut self.inner)` in a `mut self` method
when the field has a `Default`. The `rust-discipline` skill, when it is installed, owns the guard
design, the four escape hatches (`ManuallyDrop` included), and drop order.

### E0184 and E0367: the two `Drop` impl rules

`#[derive(Copy)]` plus `impl Drop` reports "the trait `Copy` cannot be implemented for this type;
the type has a destructor". A bitwise copy plus a destructor is a double free by construction, so
a resource handle is never `Copy`. Remove the derive.

`impl<T: Clone> Drop for Foo<T>` where the struct is `struct Foo<T>(T)` reports E0367, "`Drop` impl
requires `T: Clone` but the struct it is implemented for does not", with "note: the implementor
must specify the same requirement". Drop glue must exist for every instantiation, so the impl may
not apply to only some of them. Move the bound onto the struct definition:

```rust
pub struct Bar<T: Clone>(pub T);
impl<T: Clone> Drop for Bar<T> {
    fn drop(&mut self) {}
}
```

## E0631: a function item is not a coercion site

```text
error[E0631]: type mismatch in function arguments
    = note: expected function signature `fn(&String) -> _`
               found function signature `fn(&str) -> _`
help: consider wrapping the function in a closure
```

Deref coercion runs at an expression, not at a trait bound. With `v: Vec<String>` and `fn
count_words(s: &str)`, the call `count_words(&v[0])` compiles because `&String` coerces to `&str`
at that call. `v.iter().map(count_words)` fails, because `Iterator::map` demands
`F: FnMut(&String) -> B` and rustc matches the function item's signature against that bound
exactly.

```rust
fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

pub fn totals(v: &[String]) -> (usize, usize) {
    let closure: usize = v.iter().map(|s| count_words(s)).sum();
    let adapter: usize = v.iter().map(String::as_str).map(count_words).sum();
    (closure, adapter)
}
```

Inside a closure body the call is an expression again, so the coercion applies. The rule holds for
`Option::map`, `Result::map_err`, and every other higher-order call. rustc also reports E0599 on
the `.sum()` after the failed `map`; that error is a cascade and goes away with the fix.

## Related

- [SKILL.md](../SKILL.md): the triage table and the fixes that hide the bug.
- The `rust-callback-bounds` skill, when it is installed: the E0309, E0621, E0502 cascade that
  follows from adding a lifetime to a `Fn(&T) -> K` bound, and E0562.

# Type-level API techniques

Ways to move a check from run time to compile time. Each entry states what the technique buys,
what it costs, and when the cost is not worth paying. Every example compiles on rustc 1.98.1,
edition 2024.

The order is by how often the technique is the right answer. Read the first two before the rest;
they cover most cases, and the later ones are easy to over-apply.

Contents:

- Newtype for an invariant
- `#[non_exhaustive]` on anything a downstream crate matches
- Sealed trait
- Typestate
- `const fn` and const generics
- Assert an invariant at compile time
- Let the compiler narrow control flow
- When not to reach for any of these

## Newtype for an invariant

The cheapest technique, and the one that is nearly always right. A validated value gets its own
type, the constructor is the only way in, and no later code repeats the check.

```rust,run
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroPort(u16);

impl NonZeroPort {
    /// Returns `None` for port 0, which means "ask the OS for an ephemeral port"
    /// and is outside this type's non-zero domain contract.
    pub fn new(value: u16) -> Option<Self> {
        (value != 0).then_some(Self(value))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

fn main() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("bind an ephemeral loopback port");
    let assigned = listener.local_addr().expect("read assigned port").port();
    assert_ne!(assigned, 0);
    assert!(NonZeroPort::new(assigned).is_some());
    assert!(NonZeroPort::new(0).is_none());
}
```

Keep the field private. A `pub struct NonZeroPort(pub u16)` proves nothing:
any code can build an invalid one, and the type is then only a comment. See the
`rust-serde` skill for the `#[serde(try_from = "..")]` form that applies the same
rule at a deserialization boundary.

## `#[non_exhaustive]` on anything a downstream crate matches

Marks a struct or an enum as open to future additions. Downstream code cannot construct the
struct with a literal, and cannot write a match without a wildcard arm, so adding a variant or a
field later is not a breaking change.

```rust
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    NotFound,
    PermissionDenied,
}

#[non_exhaustive]
#[derive(Debug, Default)]
pub struct Options {
    pub retries: u32,
}
```

Downstream code also cannot write `Options { retries: 3, ..Default::default() }`: that is a
struct expression too, and Rust 1.99.1 rejects it with E0639. Give a `#[non_exhaustive]` struct a
constructor or a builder, or derive `Default` and let the caller assign fields on a `let mut`
value (`let mut o = Options::default(); o.retries = 3;`).

Apply it to every public error enum and every public options struct, from the first release. It
is free before publication and a breaking change to add afterwards. The restriction applies
outside the defining crate. Inside that crate, keep exhaustive matches without a wildcard so
a new variant requires a decision at every match:

```rust,run
#[non_exhaustive]
pub enum Status { Ready, Stopped }

fn code(status: Status) -> u8 {
    match status { Status::Ready => 0, Status::Stopped => 1 }
}

fn main() {
    assert_eq!(code(Status::Ready), 0);
    assert_eq!(code(Status::Stopped), 1);
}
```

This probe checks the defining crate only. Test an external consumer separately when changing
the public construction or matching contract. See the
[Reference](https://doc.rust-lang.org/1.100.0/reference/attributes/type_system.html#the-non_exhaustive-attribute).

## Sealed trait

A trait a downstream crate can name but cannot implement. Use it when the trait exists to
enumerate a closed set, or when a blanket impl would otherwise be a semver hazard.

```rust
mod sealed {
    pub trait Sealed {}
}

pub trait Codec: sealed::Sealed {
    fn name(&self) -> &'static str;
}

pub struct Json;
impl sealed::Sealed for Json {}
impl Codec for Json {
    fn name(&self) -> &'static str {
        "json"
    }
}
```

`sealed::Sealed` is public inside a private module, so a downstream crate can neither name it nor
implement it, and therefore cannot implement `Codec`. Going from sealed to open later is free.
Going from open to sealed is a breaking change, so seal by default when the set is meant to be
closed.

## Typestate

Encodes a state machine in the type, so an invalid transition does not compile. The states are
zero-sized, so it costs nothing at run time.

```rust
use std::marker::PhantomData;

pub struct Closed;
pub struct Open;

// `fn() -> S` keeps the door `Send`, `Sync`, and covariant in `S`, whatever the
// tag is. `PhantomData<S>` would inherit the tag's auto traits instead.
pub struct Door<S> {
    _state: PhantomData<fn() -> S>,
}

// Hand-written, and only on the closed state. `#[derive(Default)]` would add
// `S: Default` to the impl; `impl<S> Default for Door<S>` would hand out a
// `Door<Open>` and defeat the state machine.
impl Default for Door<Closed> {
    fn default() -> Self {
        Door { _state: PhantomData }
    }
}

impl Door<Closed> {
    pub fn new() -> Self {
        Door { _state: PhantomData }
    }

    pub fn open(self) -> Door<Open> {
        Door { _state: PhantomData }
    }
}

impl Door<Open> {
    // Only exists on an open door. Calling it on a closed one is a type error.
    pub fn walk_through(&self) {}

    pub fn close(self) -> Door<Closed> {
        Door { _state: PhantomData }
    }
}
```

The cost is real, so weigh it:

- Every transition consumes `self`, so the value cannot be stored in a field that outlives one
  state, and it cannot be held behind an `Arc`.
- The state cannot be chosen at run time. A value whose state depends on input needs an enum
  wrapper, which puts the run-time check back.
- The type name appears in every signature that touches it, including the caller's.

Two rules keep the tag from leaking into the machine, and the block above follows both. First,
`PhantomData<S>` makes the struct behave as if it owns an `S` for auto traits, variance, and drop
check, so a tag that is never constructed still decides whether the machine is `Send`. Measured
with `struct Tag(*const u8)`: a door whose field is `PhantomData<S>` fails a `Send` bound at
`Door<Tag>` with `error[E0277]: '*const u8' cannot be sent between threads safely` and
`note: required because it appears within the type 'PhantomData<Tag>'`. `PhantomData<fn() -> S>`
keeps the same covariance in `S`, and a function pointer is always `Send + Sync`, so the tag stops
deciding. Second, a derive on the struct adds `S: Trait` to the generated impl even when the only
field is the marker. `#[derive(Default)]` on the door then makes `Door::<Closed>::default()` fail
with `error[E0599]: the associated function or constant 'default' exists for struct 'Door<Closed>',
but its trait bounds were not satisfied` and `note: trait bound 'Closed: Default' was not
satisfied`. Write the impls by hand.

Use `PhantomData<fn() -> S>` only for a tag that is never constructed. For a parameter that stands
for owned data use `PhantomData<S>`, and for one that stands for a borrow use `PhantomData<&'a S>`.

A fallible transition must hand the old state back. The transition takes `self` by value, so a
plain error type destroys the value: the caller cannot retry, cannot log the old state, and cannot
fall back, because the receiver moved into the call. A retry after
`fn authenticate(self, ..) -> Result<Session<Authenticated>, AuthError>` fails with
`error[E0382]: use of moved value: 's'`. Type the error arm as `Self` and write `Err(self)`. The
cost is that the `Result` is at least as large as the state type; box the error arm only when a
size assertion proves it matters.

Give the struct a default state parameter, and keep one inherent impl for the methods that exist in
every state. Without the default, `fn audit(s: Session)` fails with `error[E0107]: missing generics
for struct 'Session'`, so every downstream signature carries a generic argument for a parameter the
caller does not choose. The default does not change constructor resolution: `Session::new()`
resolves because exactly one inherent impl defines `new`. Keep it that way. A second inherent `new`
on another state's impl makes every unqualified call fail with `error[E0034]: multiple applicable
items in scope`, and that break reaches crates that never mention the new state, so give each
state's constructor a distinct name.

```rust
use std::marker::PhantomData;

pub struct Anonymous;
pub struct Authenticated;

// `= Anonymous` lets a signature name the bare type `Session`.
pub struct Session<S = Anonymous> {
    id: u64,
    _state: PhantomData<fn() -> S>,
}

// One inherent impl for the methods that exist in every state.
impl<S> Session<S> {
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Session<Anonymous> {
    // The only inherent `new` on the whole type. A second one on another
    // state's impl makes every `Session::new()` call fail with E0034.
    pub fn new(id: u64) -> Self {
        Session { id, _state: PhantomData }
    }

    // The Err arm hands the receiver back, so the caller can retry.
    pub fn authenticate(self, secret: &str) -> Result<Session<Authenticated>, Self> {
        if secret.is_empty() {
            return Err(self);
        }
        Ok(Session { id: self.id, _state: PhantomData })
    }
}

// No generic argument here, because of the default.
pub fn audit(session: &Session) -> u64 {
    session.id()
}
```

A default type parameter is not a bound relaxation. `Session` still applies every bound the
declaration states, and a turbofish is still needed wherever inference has no other anchor.

Use typestate for a builder that must not be finished twice, or a protocol handshake inside one
function. Do not use it for a long-lived object stored in a collection.

## `const fn` and const generics

`const fn` lets a computation run at compile time. Const generics let a length be part of the
type.

```rust
pub const fn header_len(payload: usize) -> usize {
    payload + 8
}

pub struct Frame<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> Frame<N> {
    pub const CAPACITY: usize = header_len(N);

    pub fn new() -> Self {
        Frame { bytes: [0; N] }
    }

    pub fn capacity(&self) -> usize {
        Self::CAPACITY
    }
}
```

`const fn` is nearly free: mark a function `const` when its body allows it, and callers gain the
option of using it in a constant. It is not a breaking change to add, and it is a breaking change
to remove.

Const generics are the opposite. A `const N: usize` parameter propagates into every signature
that touches the type, and each distinct `N` is a separate monomorphization. Use them for a
fixed-size buffer whose size the caller genuinely knows at compile time. For a size the caller
learns at run time, use a slice and check the length once. The `rust-performance` skill owns
the monomorphization cost; measure it with `cargo llvm-lines --locked --release`.

## Assert an invariant at compile time

When a layout or a size assumption is load bearing, state it where it will fail the build:

```rust
#[repr(C)]
pub struct Header {
    kind: u32,
    length: u32,
}

// Fails to compile if the layout ever changes.
// 8 bytes, align 4, measured on rustc 1.97.0.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Header>() == 8);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::align_of::<Header>() == 4);
```

Gate the assertion on the target whose layout you measured. A size that depends on a pointer, a
`usize`, or a platform-dependent alignment differs per target, so an ungated assertion that
passes on a 64-bit host fails the build with E0080 when the same file is cross-compiled to
`i686-unknown-linux-gnu`.

This costs nothing at run time and needs no dependency. `size_of` and `align_of` are in the
prelude since Rust 1.80. On an older MSRV, qualify them with `std::mem::`. Put one
assert next to every type whose size or alignment another language depends on.
The `rust-unsafe` skill owns the layout rules these assertions protect; a type another language
reads needs `#[repr(C)]`.

## Let the compiler narrow control flow

Two forms that remove a nesting level and make the failure path explicit.

```rust
// let-else: bind, or leave. The else block must diverge.
pub fn parse_port(text: &str) -> u16 {
    let Ok(value) = text.parse::<u16>() else {
        return 0;
    };
    value
}

// let chains (edition 2024, rustc 1.88+): bind and test in one condition, no nesting.
pub fn difference(left: Option<u32>, right: Option<u32>) -> u32 {
    if let Some(a) = left
        && let Some(b) = right
        && a > b
    {
        a - b
    } else {
        0
    }
}
```

Let chains (`if let Some(x) = a && x > 0`) need edition 2024 and rustc 1.88+; the
`rust-pattern-semantics` skill, when it is installed, has the gate errors and the fallback.

## When not to reach for any of these

A run-time check is the right answer when the value is decided at run time, when the state must
be stored, or when the invariant is local to one function. The techniques above trade
flexibility for a compile-time guarantee. Spend that trade on an invariant that is genuinely
load bearing across a module boundary, not on one a single `assert!` already covers.

## Related

- [type-and-trait-traps.md](type-and-trait-traps.md) — blanket impls and borrowed cache lifetimes
- [ownership-and-concurrency-traps.md](ownership-and-concurrency-traps.md) — reference cycles,
  mutex poisoning, and builder receivers
- [SKILL.md](../SKILL.md) — the API design rules these techniques implement

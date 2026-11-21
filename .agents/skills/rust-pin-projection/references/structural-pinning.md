# Structural pinning obligations

Deep material for `rust-pin-projection`. `SKILL.md` states each obligation as a rule. This file
holds the failure each one prevents, the Miri report it produces, and the shapes that break it.
Measured on rustc 1.99.1, edition 2024, pin-project 1.1.13, pin-project-lite 0.2.17, and Miri on
nightly 2026-05-15 with the default Stacked Borrows model.

Contents:

- [The four obligations](#the-four-obligations)
- [Obligation 1: structural Unpin](#obligation-1-structural-unpin)
- [Obligation 2: pinned destruction](#obligation-2-pinned-destruction)
- [Obligation 3: notice of destruction](#obligation-3-notice-of-destruction)
- [Obligation 4: no move-out operation](#obligation-4-no-move-out-operation)
- [A named stack binding](#a-named-stack-binding)

## The four obligations

A pinning projection is a method with the shape `Pin<&mut Self> -> Pin<&mut Field>`. It declares
the field **structurally pinned**: the field is pinned whenever the struct is. Write it with
`Pin::map_unchecked_mut`. A field that is not structurally pinned projects to a plain `&mut`
through `Pin::get_unchecked_mut`. Choose per field, and state the choice in the `SAFETY:`
comment. Read `references/pin-projection-macros.md` (section "Project two fields by hand")
for a run block that projects one pinned and one unpinned field, with both `SAFETY:` comments.

The choice creates four obligations. Break one, and safe callers get undefined behaviour.

## Obligation 1: structural Unpin

The struct may be `Unpin` only when every structurally pinned field is `Unpin`. The auto impl
already does that. The defect is a hand-written one:
`impl<T> Unpin for Wrapper<T> {}` on a `Wrapper<T>` whose `inner` method projects
`Pin<&mut Self>` to `Pin<&mut T>`. Read `references/pin-projection-macros.md` (section "A
hand-written Unpin impl") for the full type.

`impl Unpin` needs no `unsafe`, so this line passes an `unsafe`-focused review. It makes
`Pin::get_mut` safe for `Wrapper<T>`, and safe callers then swap pinned data with
`mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut())`. Miri reports `allocN has been freed, so
this pointer is dangling` at the next read of the self-pointer after the swapped partner drops.
When the read comes directly after the swap, it reports `tag does not exist in the borrow stack
for this location` and names the `mem::swap` line as the write access that invalidated the tag.

Never write `impl Unpin` by hand on a type that has a pinning projection. Delete the impl, and
let the auto trait decide.

## Obligation 2: pinned destruction

`fn drop(&mut self)` receives `&mut self` even for a value that was pinned. A destructor can
therefore move a structurally pinned field out in plain safe code, for example with
`mem::replace`, and Miri reports a dangling self-pointer at the next read.

Write the destructor as if the receiver were `Pin<&mut Self>`. Both macros enforce that: a manual
`impl Drop` beside a projection macro gives `error[E0119]: conflicting implementations of trait`
on a generated `MustNotImplDrop` trait (`pin-project` prefixes the struct name). Use
`PinnedDrop`. For a hand-written type, put the body in an inner function that takes the pin. Read
`references/pin-projection-macros.md` (sections "A destructor that moves a pinned field" to
"Pinned destruction without a macro") when you write or review a destructor on a pinned type.

The same obligation rules out `#[repr(packed)]`: drop glue for a packed struct can move a field to
an aligned place before it drops it. rustc does not warn, and `pin-project-lite` rejects a packed
struct only when a field needs alignment above 1 (`E0793`). Reject packed layouts yourself. Read
`references/pin-projection-macros.md` (section "repr(packed): only one crate rejects it") when a
pinned type is packed.

## Obligation 3: notice of destruction

For a `!Unpin` target, `Pin` also guarantees that the storage is not deallocated, overwritten, or
repurposed until the destructor of the value has run or panicked. Intrusive designs, such as a
waiter queue that links futures by address, depend on this half. Three shapes break it, and none
frees memory in an obvious way:

- `Pin<Box<ManuallyDrop<T>>>` projected down to `Pin<&mut T>`. `ManuallyDrop` inhibits the
  destructor, so it never runs. `std` states that this can never be made sound.
- An `Option` that holds a pinned value and is overwritten with `None` without running the
  destructor of the value, for example through `ptr::write`. `Pin::set(None)` is sound: it
  drops the old value in place.
- `Vec::set_len`, used to shrink a `Vec` that holds pinned values. The elements above the new
  length are never destructed, and their storage becomes reusable.

Unsafe code that manages its own storage must call `ptr::drop_in_place` before it frees or
reuses that storage. A destructor that can panic part way through, and so skip the remaining
destructors, must abort the process instead. The `rust-panic-safety` skill, when it is
installed, covers that abort; by default, make each `Drop` infallible.

## Obligation 4: no move-out operation

If any operation you expose can move the field out of a pinned value, that field cannot be
structurally pinned: `Option::take`, `mem::replace`, and `mem::swap` are the usual ones. The type
system does the work once the projection returns `Pin<&mut Option<F>>`, because `Pin<Ptr>`
implements `DerefMut` only for an `Unpin` target:

```rust,compile_fail,E0596
use pin_project_lite::pin_project;
use std::pin::Pin;

pin_project! { struct WithPin<F> { #[pin] fut: Option<F> } }

fn steal<F>(s: Pin<&mut WithPin<F>>) -> Option<F> {
    s.project().fut.take()      // E0596
}
```

The help line says that `DerefMut` is required to modify through a dereference, but is not
implemented for `Pin<&mut Option<F>>`. Delete the `#[pin]` attribute, and the identical body
compiles, because the projection then hands out `&mut Option<F>`, and the move of pinned data is
silent. The same `E0596` guards `mem::swap` on two `Pin<&mut S>` values.

## A named stack binding

`Pin::new_unchecked(&mut local)` commits the **value** for the rest of its life, not for the
lifetime of the `Pin`. The `Pin` is an ordinary value that holds an ordinary `&mut`. Its drop
releases the borrow, and the borrow checker then gives by-value access back to the local. A
function body `let mut s = SelfRef::new(0xAA); unsafe { Pin::new_unchecked(&mut s) }.init(); s`
moves the pinned value out. `cargo build`, `cargo test`, and `cargo clippy -- -W clippy::pedantic`
accept that function with no diagnostic. Miri reports `allocN has been freed, so this pointer is
dangling` at the read through the self-pointer after the move. Section "A named stack binding
under Miri" of `pin-projection-macros.md` has the full report.

A tutorial `macro_rules!` that shadows the caller's binding with
`let mut $name = Pin::new_unchecked(&mut $name);` fails with `E0133` at a safe call site. The
obvious fix, `unsafe { pin_hand!(a); }`, compiles and is worse: the shadow dies at the closing
brace, and the original binding moves out. Section "A hand-rolled pinning macro" of
`pin-projection-macros.md` shows the program.

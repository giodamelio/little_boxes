---
description: Use when designing or reviewing Rust API shape (pub signatures, trait bounds, blanket and sealed impls, newtypes, Drop or RAII guards), or when method resolution or trait coherence fails (method ambiguity, autoderef, UFCS, E0034, E0119 blanket impl overlap, E0117 orphan rule). Also covers unwrap and expect, lock poisoning, Condvar::wait_while, wildcard arms, non_exhaustive enums, and a pre-merge semver review of such a diff. Not for panics at an FFI boundary (rust-panic-safety).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-discipline
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 3d8e6ff026947de9d122bd62e2bf598f45c08d2d
name: rust-discipline
---
# Rust Discipline

Review rules for Rust API shape: `pub` and `pub(crate)` signatures, struct definitions, trait
bounds, and trait impls. Each rule targets a defect that compiles and then breaks a caller, a
downstream crate, or a later release. Apply them to every changed signature in the diff.

| Severity | Meaning |
|----------|---------|
| CRITICAL | Blocks the merge. The defect is silent, or it breaks downstream code and a release cannot undo it. |
| WARNING  | Fix it, or justify it in one line in the diff. |

## Verify

| Claim | Check | What a green result does not prove |
|-------|-------|------------------------------------|
| No lint regression | `cargo clippy --all-targets --locked -- -D warnings` | Clippy denies `derived_hash_with_manual_eq` by default. It does not see two manual `Hash` and `PartialEq` impls that disagree. |
| A public API change keeps SemVer | `cargo semver-checks check-release` on a published crate, if it is installed. Pass `--baseline-rev <rev>` for an unpublished one. | Version 0.50.0 (rustc 1.98.1) misses `&str` changed to `impl AsRef<str>`, a new blanket impl on a public trait, and a new provided method whose name can collide (E0034). Review those by hand. |
| A bound accepts every promised caller | A test that passes each caller shape the API promises. For a `for<'a>` bound, pass the function or method path itself. | Only the shapes the test passes. |
| A hash key keeps the `Eq`/`Hash` contract | A test that inserts through one form and looks up through the other | Only the forms the test uses. |

Update cargo-semver-checks with each stable rustc: it reads rustdoc JSON, whose format changes.
The `rust-crate-release` skill owns the bump decision from `cargo semver-checks` output.

## References

| Reference | Read it when the diff | Or rustc reports |
|-----------|-----------------------|------------------|
| [type-and-trait-traps.md](references/type-and-trait-traps.md) | adds a trait impl, a newtype, a lifetime parameter, or a blanket impl on a public trait | E0038 |
| [ownership-and-concurrency-traps.md](references/ownership-and-concurrency-traps.md) | picks a builder receiver, passes a large value, clones a handle, shares state (`Arc`, `Weak`, a mutex, a `Condvar`), or adds arithmetic on input | |
| [trait-resolution.md](references/trait-resolution.md) | adds a `Deref` impl, an extension trait, a `downcast_ref` chain, a forwarding or blanket impl, a second conversion impl on one type pair, or a foreign trait impl on a wrapper | E0034, E0119, E0117; a call that autoderef sends to the wrong impl |
| [data-shape-traps.md](references/data-shape-traps.md) | adds a hash key or an `Ord` impl, reverses or truncates text, or builds a large array for the heap | |
| [argument-shapes.md](references/argument-shapes.md) | picks a parameter, callback, or return shape on a `pub` signature, or makes a concrete parameter generic | E0308 or `not general enough` at a callback |
| [drop-and-raii.md](references/drop-and-raii.md) | adds a `Drop` impl, a guard type, or cleanup that must survive a panic | E0509, E0507 in `drop` |
| [type-level-api.md](references/type-level-api.md) | adds a newtype, a sealed trait, a typestate, a `#[non_exhaustive]` type, or a `const` generic | |
| [api-conventions.md](references/api-conventions.md) | names a `pub` method, constructor, getter, or conversion, or derives the traits of a `pub` type | |
| [review-checklist.md](references/review-checklist.md) | is under a full review pass or a merge gate; for one signature, use its API design group | |
| [topic-owners.md](references/topic-owners.md) | touches a topic outside these rules: performance, atomics, async, `unsafe`, FFI, serde, lints, release | |

[API design](#api-design) covers E0562 and E0596 at a stored callback and E0277 `expected an
'FnMut(&&T)' closure`.

## API design

### Accept borrowed args, not owned references

**Severity: WARNING**

Accept `&str`, `&[T]`, and `&Path` instead of `&String`, `&Vec<T>`, and `&PathBuf`: the owned
shapes force the caller to hold an allocation. Find violations with
`rg ':\s*&(String\b|Vec<|PathBuf\b)' --type rust -n`, and keep `&String` or `&Vec<T>` only for the
[two real exceptions](references/argument-shapes.md#when-string-and-vect-are-correct).

Take a generic `<P: AsRef<Path>>` only when call-site convenience matters more than one body per
type, and keep the body in a private concrete function. A published reference parameter that
becomes generic breaks every caller that coerces the function to a `fn(&T)` pointer (E0308) or
passes it to an `Fn(&T)` bound (`not general enough`). Read
[argument-shapes.md](references/argument-shapes.md) before you change a published shape.

Keep an iterator's extra reference out of a public bound. `items.iter().find(pred)` under
`F: Fn(&T) -> bool` fails with `E0277: expected an 'FnMut(&&T)' closure`. Do not widen the bound
to `Fn(&&T)`, which pins `&&T` into the API; re-borrow: `items.iter().find(|item| pred(item))`.

### Return a slice, not the container type

**Severity: WARNING**

A published `-> &mut Vec<T>` accessor pins `Vec`: a later switch to `Box<[T]>` breaks every
caller (E0599). Return `&[T]` or `&mut [T]`, and name each length-changing operation as a method
([example](references/api-conventions.md#return-a-slice-not-the-container-type)).

### Store a callback as `Box<dyn Fn>`, not `Box<dyn FnMut>`

**Severity: WARNING**

`impl Fn` in a field fails with `E0562`, and a `Vec<F>` cannot hold two closure types. Store
`Box<dyn Fn(..)>`. `Box<dyn FnMut(..)>` fails `publish(&self)` with `E0596`, and the `&mut self`
fix spreads to every caller and blocks sharing the owner behind an `Arc`. Put the mutation in the
callback's own `Cell` or `Mutex`. Clone captured data for the implicit `'static` bound; do
not store `Weak` callbacks, which die silently when the caller releases its `Arc`
([example](references/argument-shapes.md#store-a-callback-as-boxdyn-fn-not-boxdyn-fnmut)).

### Do not store `&'a mut H` in a struct field

**Severity: WARNING**

A `&'a mut H` field puts `'a` on the struct, on every holder, and on every signature that returns
it; removing it from a published type later breaks callers. Take a generic `H`, so an
owning caller (`Box<dyn Handler>`) names no lifetime, and add one forwarding impl per pointer the
caller may pass. Each one needs `+ ?Sized` (else `dyn Handler` fails with E0277) and the body
`H::handle(self, r)` (`self.handle(r)` overflows the stack). Read
[trait-resolution.md](references/trait-resolution.md#a-pointer-forwarding-impl-needs-sized-and-a-body-that-names-the-inner-impl)
before the trait ships. Find candidates with `rg "struct .+<'.+>\s*\{" --type rust -n`.

### Do not tie a callback argument to a named lifetime

**Severity: WARNING**

`F: Fn(&T) -> R` is already higher-ranked, so the callback cannot keep the reference: `E0521`
on a closure that stashes it shows the bound is correct. The defect is
`fn visit<'a, F: Fn(&'a T)>(..)`: it accepts a closure that stores the reference silently (1.99.1). Write `F: Fn(&T)`, or `for<'a>` where
the bound must name `'a`. For a callback that returns a borrow of its argument, write
`F: Fn(&T) -> &K` with `K: ?Sized`. Read
[argument-shapes.md](references/argument-shapes.md#callback-bounds-past-the-plain-borrow)
when you pick a callable bound or pass a method path to a `for<'a>` bound.

## Panics and error returns

This section is the catalog's home for the `unwrap` and `expect` rule.

- Use `.unwrap()` in tests and `examples/` only. Elsewhere, propagate with `?`.
- Use `.expect("<invariant>")` in non-test code only for a real invariant. The message states
  what must be true: `"config validated at startup"`. `"should never fail"` states a wish.
- State the lock-poisoning policy at every `lock()`. Write `.expect("<name> mutex poisoned")`
  when a panic under the lock must also stop the next user. Write
  `.unwrap_or_else(PoisonError::into_inner)` when the data stays valid after a panic mid-update.
  A bare `.lock().unwrap()` hides the choice. `parking_lot` and `tokio::sync` mutexes do not
  poison ([migration](references/ownership-and-concurrency-traps.md#parking_lot-and-tokio-mutexes-do-not-poison-on-panic)).
- Give `panic!`, `unreachable!`, and `todo!` a reason (`unreachable!("kind {kind:?} filtered
  earlier")`), and use them only for cases the code makes impossible.
- Return `Result` from a library path that can fail; never build control flow on a panic.
- Enforce the `unwrap` half with `clippy::unwrap_used` and `allow-unwrap-in-tests = true`. The
  `rust-lints` skill holds the rollout and the `#![expect]` that an `examples/` file needs.

The `rust-error-handling` skill owns the error type choice (`thiserror` or `anyhow`). Use one error
convention per dependency chain: one shared error type, or one enum per crate that the caller
translates, never both.

## Drop and RAII

- Prefer `std::os::fd::OwnedFd` and `std::os::windows::io::OwnedSocket` over a raw `i32` or
  `SOCKET`, which leaks on every error path that does not close it.
- Fields drop in declaration order, after `Drop::drop`. Document that order on a
  cleanup-critical type.
- `impl Drop` blocks every move of a non-`Copy` field out of the struct (E0509). Put `Drop` on a
  one-field guard, and keep the aggregate `Drop`-free. Make the guard's field an `Option` only
  when the cleanup consumes it.
- Use `core::mem::DropGuard` on Rust 1.100+, or a local `Drop` type, for cleanup. Under
  `panic = "abort"` no destructor runs after a panic.
- Never argue soundness from a guard's `Drop`. `mem::forget` is safe, so a caller can skip it.

## Match exhaustiveness

- **No `_ =>` wildcard on a crate-private enum.** A wildcard absorbs a new variant silently. List
  every arm, and join arms with identical handling as `A | B =>`. Enforce it with
  `#![warn(clippy::wildcard_enum_match_arm)]` on the modules that match local enums, and put
  `#[expect(clippy::wildcard_enum_match_arm, reason = "...")]` on a match of a foreign
  `#[non_exhaustive]` enum such as `io::ErrorKind`, where `_` is required.
- Mark a public enum that downstream code matches `#[non_exhaustive]` in its first release.
  Adding the attribute later is a breaking change.
- **A `downcast_ref` chain skips a new implementor silently.** For a closed set of types, use an
  enum, which fails with `E0004` instead. `if let` and `let else` stay fine for one variant.

## Locks and condition variables

- Write the lock order on the struct (`// Lock order: a -> b -> c`); every nested acquisition
  follows it.
- Measure contention before you switch to `parking_lot` or pick `RwLock` over `Mutex`.
- **Wait on a `Condvar` only through a predicate**: a bare `wait` blocks for ever when the
  notify arrives first. Call `cv.wait_while(guard, |n| *n == 0)`, with a predicate that reads only
  state that the same mutex protects.
- A blocking-lock guard held across `.await` makes the future `!Send` and can deadlock the
  executor; `clippy::await_holding_lock` finds it.

## Lint suppression

Reject a new `#[allow]` where `#[expect(lint, reason = "...")]` works, and any suppression of a
`clippy::correctness` or `clippy::suspicious` finding. The `rust-lints` skill owns the
suppression order and `clippy.toml`.

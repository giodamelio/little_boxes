# Review checklist

The full-pass checklist for `rust-discipline`. Run it when a review request or a merge gate asks
for a full pass over a Rust diff. For one changed signature, apply the API design group only.
Every rule it names is stated in full in [`../SKILL.md`](../SKILL.md), in one of the other
reference files, or in the skill named on the question.

If the answer to any question is yes, revise the change before you merge it.

**API design**

1. Any `&String`, `&Vec<T>`, or `&PathBuf` parameter? Use `&str`, `&[T]`, `&Path`, or a named
   `<P: AsRef<..>>` generic.
2. Any `&'a mut Trait` stored in a struct field? Use a generic `H: Trait`, plus forwarding impls
   that carry `+ ?Sized` and delegate through `H::method(self, ..)`.
3. Any callback argument tied to a lifetime parameter of the function or struct
   (`fn f<'a, F: Fn(&'a T)>`) where the callback must not keep the reference? Write `F: Fn(&T)`;
   elision already makes it higher-ranked.
4. Any `pub` accessor that returns `&mut Vec<T>` or `&mut String`? Return `&mut [T]` or
   `&mut str`, and name each length-changing operation. Any public bound that names `&&T`?
5. Any callback stored as `Box<dyn FnMut>` where `Box<dyn Fn>` works? Any `Weak` callback
   registry that drops registrations silently?
6. Any `Fn` bound picked from the `move` keyword instead of the body? Any callback method whose
   lifetime sits on the `impl` block instead of the method?
7. Any published concrete parameter made generic? Ship it in a major version. A reference
   parameter (`&str`, `&Path`, `&[T]`) then breaks every caller that coerces the function to a
   `fn(&T)` pointer or passes it to an `Fn(&T)` bound. Any parameter can break type inference at a
   call site. cargo-semver-checks flags neither.
8. Any new `pub unsafe fn` where a safe wrapper can hold the invariant? Keep the `unsafe fn`
   `pub(crate)` and export the safe wrapper. A `pub unsafe fn` moves the soundness proof to every
   caller [`rust-unsafe`].

**Panics, errors, and resources**

9. Any new `.unwrap()` outside tests and `examples/`, any `.expect()` whose message states no
   invariant, or any `.lock()` with no stated poisoning policy?
10. Any `Box<dyn std::error::Error>` returned from a library crate? [`rust-code-style`]
11. Any raw `i32` file descriptor held across an error path? Any `Drop` impl with no documented
    field order?
12. Any `_ =>` arm in a match over a crate-private enum? Any `downcast_ref` chain over a closed
    set of types?

**Concurrency, unsafe, FFI, and lints** (owner skills in brackets)

13. Any allocation inside an event-loop tick, a per-item decode loop, or a parser hot path? Declare
    the buffer outside the loop and `clear` it each pass [`rust-hot-path`].
14. Any blocking-lock guard (`std::sync::Mutex` or `RwLock`, `parking_lot`) held across
    `.await`? `clippy::await_holding_lock` finds them. Any blocking syscall inside async code with
    no `spawn_blocking` and no dedicated thread [`rust-async-internals`]?
15. Any `Condvar::wait` outside a `wait_while` call or a predicate loop? Any nested lock
    acquisition that breaks the documented lock order?
16. Any new atomic ordering with no comment that names the data it publishes? Any `Relaxed` on a
    publish/subscribe flag [`memory-model`]?
17. Any `unsafe` block with no `// SAFETY:` comment, or any `unsafe fn` with no `# Safety`
    section [`rust-unsafe`]?
18. Any FFI entry point with no `catch_unwind` that can panic instead of mapping a typed Rust
    error to an ABI-safe status, sentinel, out-parameter, or foreign exception
    [`rust-panic-safety`, `ffi-error-progress-cancel`]?
19. Any new `#[allow]` where `#[expect(lint, reason = "...")]` works? Any suppressed
    `clippy::correctness` or `clippy::suspicious` finding? [`rust-lints`] Any new `deny.toml`
    exception with no reason [`rust-security`]?

**Trait and type-system traps** (details in
[`type-and-trait-traps.md`](type-and-trait-traps.md),
[`ownership-and-concurrency-traps.md`](ownership-and-concurrency-traps.md),
[`trait-resolution.md`](trait-resolution.md), and
[`data-shape-traps.md`](data-shape-traps.md))

20. Any `impl Drop` on a struct where a field must be consumed? Move `Drop` onto a one-field
    guard; make its field an `Option` only when the cleanup consumes it. Use `ManuallyDrop` only
    when `size_of` proves a saving.
21. Any `fn(T) -> T` that takes a struct past the target's inline-copy boundary on a hot path
    [`rust-hot-path`]?
22. Any manual `PartialEq` next to a derived `Hash`, or two manual impls that normalize
    differently, on a `HashMap` or `HashSet` key? Any manual `PartialOrd` that does not return
    `Some(self.cmp(other))` on a type that also implements `Ord`?
23. Any `#[derive(Clone)]` on a struct that contains `Arc<T>` where the caller might expect an
    isolated copy?
24. Any `Deref` impl on a newtype that is not a smart pointer or a read-only `[T]` or `str`
    view of a collection newtype? Any `Deref` relied on to satisfy
    a trait bound or a `dyn Trait` coercion? Neither one walks the deref chain.
25. Any migration from `std::sync::Mutex` to `parking_lot` or `tokio::sync::Mutex` that relied
    on poison detection?
26. Any unchecked arithmetic on a value derived from untrusted input?
27. Any `Arc<T>` that points back to its parent container?
28. Any cache that stores input references past the source owner's lifetime? Keep the owner
    alive, shorten the cache's scope, or store owned data. Splitting lifetime names does not
    extend the source's life.
29. Any `impl<T: ...> PubTrait for T` on a public trait that is not sealed? Seal the trait, or
    write explicit per-type impls. The same blanket impl also makes every pointer-forwarding
    impl `E0119`, and blocks every later concrete impl on the same `Self` type.
30. Any foreign trait implemented for `Rc<Local>`, `Arc<Local>`, or `Vec<Local>`? Only `&T`,
    `&mut T`, `Box<T>`, and `Pin<P>` are `#[fundamental]`; the rest is `E0117`. Newtype the
    wrapper.
31. Any `Box::new([T; N])`, or any return of `[T; N]` by value, where the array is large enough
    for `clippy::large_stack_arrays` or `clippy::large_stack_frames` to flag? Build it
    with `vec![x; n].into_boxed_slice()`, or collect an exact-length iterator into `Box<[T]>`.
32. Any extension-trait method whose name already exists on the type, or on a type in its deref
    chain? The shadowing is silent, and adding the method to a published trait breaks downstream
    builds with `E0034`.
33. Any `impl From<X> for Y` beside an `impl TryFrom<X> for Y`? The `core` blanket impl makes
    the pair `E0119`, and the choice between them is permanent.

# API conventions

The naming and trait conventions of a public Rust API, with the code of each rule in the Rust API
Guidelines (https://rust-lang.github.io/api-guidelines/checklist.html) so that a review can cite
it. Each row names the check that finds a violation. A rule that another skill or reference owns
is listed only with a pointer.

- [Naming](#naming)
- [Standard traits](#standard-traits)
- [Shape of functions and types](#shape-of-functions-and-types)
- [Return a slice, not the container type](#return-a-slice-not-the-container-type)
- [Crate documentation](#crate-documentation)
- [Rules owned elsewhere](#rules-owned-elsewhere)

This file is the catalog's home for API method naming. The `rust-code-style` skill owns case
lints, test names, and crate names.

## Naming

| Code | Rule | Check |
| --- | --- | --- |
| C-CASE | `UpperCamelCase` for types, traits, and variants; `snake_case` for functions, methods, modules, and locals; `SCREAMING_SNAKE_CASE` for constants and statics. An acronym is one word: `Uuid`, `HttpClient` | The rustc case lints (`rust-code-style` owns their suppression) |
| C-CONV | `as_` is free and borrows to borrows; `to_` costs work, borrowed to owned or borrowed to borrowed; `into_` consumes `self`. Put `mut` in type order: `as_mut_slice`, not `as_slice_mut` | `clippy::wrong_self_convention` checks the receiver of `as_`, `from_`, `into_`, `is_`, and `to_` |
| C-GETTER | A getter has no `get_` prefix: `fn name(&self) -> &str`, `fn name_mut(&mut self)`. Keep a bare `get` only where one obvious value exists (`Cell::get`) or for an indexed lookup that returns `Option` | `rg 'pub fn get_[a-z_]+\(&(mut )?self\)' --type rust` |
| C-CTOR | `new` is the primary constructor; `with_capacity`, `with_x`, and `try_new` are variants; `from_x` converts and pairs with a `From` impl; I/O resources use domain verbs (`File::open`, `TcpStream::connect`) | `clippy::new_without_default`, `clippy::should_implement_trait` (an inherent `from_str`, `add`, `next`) |
| C-ITER | A collection offers `iter`, `iter_mut`, and `into_iter` | `clippy::iter_without_into_iter`, `clippy::into_iter_without_iter` |
| C-ITER-TY | Those methods return types named `Iter`, `IterMut`, and `IntoIter`; `keys()` returns `Keys` | Read the `pub` return types |
| C-WORD-ORDER | One word order across the crate: verb-object-error for errors (`ParseIntError`, not `IntParseError`) | Read the `pub` names together |
| C-FEATURE | A Cargo feature has a plain name (`std`, `serde`), not `use-std` or `with-serde`. A feature only adds API; it never removes an item or changes a signature | `cargo tree -e features` shows which crate turns a feature on, and `cargo-workflows` owns unification |
| (no code) | `len` comes with `is_empty` | `clippy::len_without_is_empty` |
| (no code) | No `Manager`, `Helper`, `Util`, or `Service` name where the type has a real job to name; no module name repeated in the type (`http::Client`, not `http::HttpClient`) | `clippy::module_name_repetitions` (pedantic) for the second half |
| (no code) | No abbreviation in a public name (`connection`, not `conn`). A local binding may abbreviate when the context is clear | Read the `pub` names |

## Standard traits

A downstream crate cannot implement a standard trait for your type (the orphan rule), so a
missing impl is a defect that only you can fix.

| Code | Rule | Check |
| --- | --- | --- |
| C-COMMON-TRAITS | A public type implements, where the fields allow it: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Default`. `Copy` only for small plain values, because the compiler then copies without a word | Read the derives; `data-shape-traps.md` owns the `Hash`/`Eq` and `Ord`/`PartialOrd` agreement |
| C-DEBUG | Every public type implements `Debug`; a hand-written impl redacts secrets and lists every field (`finish_non_exhaustive` when it hides one) | `missing_debug_implementations` (rustc, allow by default); `clippy::missing_fields_in_debug` |
| C-CONV-TRAITS | Implement `From`, `TryFrom`, `AsRef`, `AsMut`; never `Into` or `TryInto`, which the blanket impls derive from `From` | `clippy::from_over_into` |
| C-COLLECT | A collection implements `FromIterator` and `Extend` | Read the `FromIterator` and `Extend` impls; the `rust-iterator-impl` skill owns their rules |
| C-NUM-FMT | A number-like type implements the `fmt` traits that apply (`Display`, `LowerHex`, ...) | Read the impls |
| C-RW-VALUE | A generic reader or writer function takes `R: Read` or `W: Write` by value; `&mut R` also satisfies the bound | Read the bounds |
| (no code) | `FromStr` instead of an inherent `from_str`, so `"..".parse()` works | `clippy::should_implement_trait` |

## Shape of functions and types

| Code | Rule | Check |
| --- | --- | --- |
| C-CUSTOM-TYPE | A `bool` parameter in a public signature becomes a two-variant enum or two methods: `Widget::new(Size::Small, Shape::Round)`, not `Widget::new(true, false)` | `clippy::fn_params_excessive_bools` with `max-fn-params-bools = 0` in `clippy.toml` (the default is 3); `clippy::struct_excessive_bools` for fields |
| C-BITFLAG | A set of independent flags uses the `bitflags` crate; an enum models exactly one choice | Read the types |
| C-BUILDER | Up to two optional parameters fit inherent constructors (`new`, `with_x`). Beyond that, or when construction validates, add `FooBuilder` with infallible setters named after the fields and `build() -> Result<Foo, Error>` that does every check | Read the constructors |
| C-NO-OUT | Return a value or a tuple, not an out-parameter `&mut T`. Reusing a caller's buffer (`Read::read`) is the exception | Read the `&mut` parameters |
| C-INTERMEDIATE | Expose the intermediate result that callers would otherwise compute twice (`binary_search` returns the insertion point in `Err`) | Read the return types |
| C-CALLER-CONTROL | Take ownership only when the function stores or consumes the value; let the caller decide when to clone | `clippy::needless_pass_by_value` |
| C-METHOD | A function with a clear receiver is a method | Read the free functions |
| C-SMART-PTR | A smart pointer has no inherent methods that could hide the pointee's; use associated functions (`Rc::clone(&x)`, `Box::leak(b)`) | Read the inherent `impl` of a `Deref` type |
| C-DEREF | `Deref` only on a smart pointer or an owning wrapper, never to fake inheritance | `trait-resolution.md` owns the traps |
| (no code) | A builder-style or pure method that returns `Self` carries `#[must_use]` | `clippy::return_self_not_must_use`, `clippy::must_use_candidate` (both pedantic) |

## Return a slice, not the container type

`fn items_mut(&mut self) -> &mut Vec<String>` puts `Vec` in the public signature. A later switch
to `Box<[String]>` breaks every caller with `E0599: no method named 'push' found for mutable
reference '&mut [String]'`, which reads like a caller mistake and ships as a patch release.
Inside one crate a `&mut Vec<T>` accessor costs nothing; apply the rule to a published API.

```rust
pub struct Order {
    items: Vec<String>,
}
impl Order {
    pub fn items(&self) -> &[String] { &self.items }
    // `&mut Vec<String>` here would pin `Vec` into the public signature.
    pub fn items_mut(&mut self) -> &mut [String] { &mut self.items }
    // Name every operation that changes the length.
    pub fn add_item(&mut self, item: String) { self.items.push(item); }
}
```

## Crate documentation

| Code | Rule | Check |
| --- | --- | --- |
| C-CRATE-DOC | The crate root opens with `//!` docs that say what the crate does and show one example | `rustdoc::missing_crate_level_docs` under `cargo doc` (the `rust-lints` strict template sets it); read the example |
| C-EXAMPLE | Every public module, type, trait, function, and macro has a rustdoc example that shows why a caller uses it | Read the docs. Stable rustdoc has no lint for it: `rustdoc::missing_doc_code_examples` is an unknown lint on Rust 1.99.1 |

## Rules owned elsewhere

| Code | Owner |
| --- | --- |
| C-GOOD-ERR (error types implement `Error`, `Send`, `Sync`; meaningful messages) | The `rust-error-handling` skill |
| C-FAILURE, C-QUESTION-MARK, C-LINK (rustdoc `# Errors`/`# Panics`/`# Safety` sections, `?` in examples, intra-doc links) | `rust-code-style`, the rustdoc contract |
| C-SEALED, C-STRUCT-PRIVATE, C-NEWTYPE, C-NEWTYPE-HIDE | `type-level-api.md` in this skill |
| C-STABLE (a stable crate's public dependencies are stable) and the SemVer evolution of a published API | `rust-crate-release`; `cargo semver-checks` is the check |
| C-SEND-SYNC (public types are `Send` and `Sync` where possible) | `rust-send-sync`; a compile-time `fn assert_send<T: Send>() {}` call is the check |
| C-DTOR-FAIL, C-DTOR-BLOCK (a destructor never fails or blocks) | `drop-and-raii.md` in this skill, "Keep blocking and fallible cleanup explicit" |
| C-SERDE (serde behind a feature named `serde`) | `rust-serde`; gate each impl with `#[cfg_attr(feature = "serde", derive(serde::Serialize))]` |

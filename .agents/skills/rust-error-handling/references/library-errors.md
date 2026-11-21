# Library errors: dependency types, opaque errors, and codes

Facts are for thiserror 2.0.21 on Rust 1.99.1 unless a section names another version.

Contents: dependency types; opaque errors; error codes; adding variants; boxed string errors; infallible results.

## Dependency error types are public API

A public error that holds or converts a third-party error names that crate in your API:

- a variant field of type `dep::Error`, with or without `#[from]`;
- a `pub` type with `impl From<dep::Error>`, which `#[from]` generates;
- `#[error(transparent)]` over `dep::Error`, which also forwards its `Display` text.

When you upgrade `dep` to a new major version, the field type and the `From` impl change. A
downstream crate that matched the field, built the variant, or relied on the conversion breaks,
so the upgrade is a major release of your crate. A caller on another major version of `dep` has
a different `dep::Error` type, and its `?` into your error fails with E0277.

Choose one shape per public error:

| Shape | Use it when | Cost |
| --- | --- | --- |
| A variant with your own data and a `#[source]` field of a std type | The cause is a std error (`io::Error`, `ParseIntError`) | None; std types are stable |
| A `#[source] Box<dyn Error + Send + Sync + 'static>` field | The cause is a dependency error that callers rarely inspect | The caller downcasts to read it; your API names no dependency |
| A `dep::Error` field or `#[from] dep::Error` | The crate is a thin wrapper over `dep`, and you re-export `dep` | Every major bump of `dep` is a major bump of your crate |

Keep the conversion from a dependency error in a private function or in an impl on a private
type, so no `pub` `From` impl names it.

## An opaque error with a kind

When the set of causes changes often, make the error a struct with private fields and a
`kind()` method that returns a small `#[non_exhaustive]` enum. `std::io::Error` and
`io::ErrorKind` follow this shape. The caller matches on the kind, reads the message, and walks
the source chain; the fields stay free to change.

```rust
use std::error::Error as StdError;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    NotFound,
    InvalidInput,
    Backend,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// A stable code for machines. Never change or reuse a published code.
    pub fn code(&self) -> &'static str {
        match self.kind {
            ErrorKind::NotFound => "not_found",
            ErrorKind::InvalidInput => "invalid_input",
            ErrorKind::Backend => "backend",
        }
    }

    // Private, so no public `From` impl names the cause's crate.
    fn backend(message: String, source: impl StdError + Send + Sync + 'static) -> Self {
        Self { kind: ErrorKind::Backend, message, source: Some(Box::new(source)) }
    }
}

pub fn open(path: &Path) -> Result<std::fs::File, Error> {
    std::fs::File::open(path)
        .map_err(|source| Error::backend(format!("cannot open {}", path.display()), source))
}
```

`Error::source()` returns the boxed cause as `&(dyn Error + 'static)`, so a caller can still
call `downcast_ref::<std::io::Error>()` on it.

The `match` in `code()` lists every kind with no wildcard: inside the defining crate,
`#[non_exhaustive]` does not require one, so a new kind fails to compile until it has a code.

## Error codes for machines

- Give each case that a program handles a stable code: a `code()` method that returns
  `&'static str`, or the variant itself for a Rust caller. A message is for people and may
  change in any release.
- Pin each published code in a test with `assert_eq!(err.code(), "not_found")`. A renamed code
  breaks every log query, alert, and client that reads it.
- Retire a code; never give it a new meaning.
- Across an FFI boundary, the `ffi-error-progress-cancel` skill owns the mapping. Without it,
  send the `code()` string and the message, and never a Rust type name or `Debug` output.

## Add a variant without a break

A new variant on a `#[non_exhaustive]` enum is a minor change. On an enum without it, every
downstream `match` with no wildcard breaks. A new field on a struct variant breaks every
downstream pattern that names the variant's fields without `..`; mark the variant itself
`#[non_exhaustive]` when it may grow fields. Run `cargo semver-checks check-release`, if it is
installed, before the release.

## String errors into std boxed errors

`String` does not implement `Error`, but std provides dedicated conversions into both boxed
error forms. `?` uses those conversions without an additional error crate:

```rust,run
use std::error::Error;

fn local() -> Result<(), Box<dyn Error>> {
    Err::<(), _>(String::from("invalid input"))?;
    Ok(())
}

fn shared() -> Result<(), Box<dyn Error + Send + Sync>> {
    Err::<(), _>(String::from("invalid input"))?;
    Ok(())
}

fn main() {
    assert_eq!(local().unwrap_err().to_string(), "invalid input");
    assert_eq!(shared().unwrap_err().to_string(), "invalid input");
}
```

This proves the std conversion and preserved message. It does not give the string a typed
error kind or make `String` satisfy the `anyhow::Error` conversion bound.

## Infallible results on Rust 1.100+

Rust 1.100 allows `!` in type arguments and makes `std::convert::Infallible`
an alias for it. Keep `Infallible` in source that must build on an older MSRV.
Do not define separate trait impls for these two names; they now name one type.

```rust,run
use std::convert::Infallible;

fn value(result: Result<u32, !>) -> u32 {
    let Ok(value) = result;
    value
}

fn main() {
    let result: Result<u32, Infallible> = Ok(42);
    assert_eq!(value(result), 42);
}
```

This probe checks the alias and the irrefutable pattern on Rust 1.100.
It does not make a fallible operation infallible. Model a real failure with a
concrete error type.

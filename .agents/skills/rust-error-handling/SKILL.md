---
description: Use when designing, propagating, or reporting Rust errors, or when rustc says "`?` couldn't convert the error" (E0277), "is not satisfied" for std::error::Error, or E0599 on .context. Covers thiserror enums with `#[from]` and `#[source]`, the Error::source chain, anyhow::Context, Box<dyn Error + Send + Sync>, one error enum per module or per crate, dependency error types in a public API, printing an error chain for users and logs, stable error codes for machines, and ExitCode or Termination in main. Not for FFI error mapping (ffi-error-progress-cancel).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-error-handling
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: c0c13d84d1ce01b888f2dae3ee5efa48dfbceee4
name: rust-error-handling
---
# Rust Error Handling

This skill owns error-type design and propagation in a library and in an application: the
type a function returns, the source chain, context, conversion with `?`, and how the program
reports a failure. Facts about crates are for thiserror 2.0.21 and anyhow 1.0.104 on Rust
1.99.1.

Other skills own the neighbors. The `rust-discipline` skill owns `unwrap` and `expect` and the
`#[non_exhaustive]` rule. The `rust-code-style` skill owns the rustdoc `# Errors` section. The
`rust-cli` skill owns the process exit-code table and stderr output. The `rust-panic-safety`
skill owns panics at an FFI boundary; the `ffi-error-progress-cancel` skill owns error shapes
across FFI. Install those skills, or, without them, map each error at an FFI boundary to a
stable `code()` string and catch each panic with `std::panic::catch_unwind`.

## Gotchas

- A `Display` message that includes its source prints the source twice in a chain.
  `#[error("io error: {0}")] Io(#[from] io::Error)` prints `io error: no such file: no such file`
  under anyhow `{:#}`. Return the cause from `source()` or print it in `Display`, never both.
- `?` calls `From::from` once. It does not chain `A -> B -> C`, even when both impls exist.
- `.map_err(|_| Error::Io)` drops the cause. Keep the original error as the `#[source]` field.
- thiserror rejects `#[from]` on a variant with another field (`deriving From requires no
  fields other than source and backtrace`). Use `#[source]` and `map_err` when the variant
  also carries context, such as a path.
- `anyhow::Error` needs a `Send + Sync + 'static` source; `std::thread::spawn` and
  `tokio::spawn` need a `Send + 'static` result. A `Box<dyn Error>` without `Send + Sync`, or
  an `Rc` in a field, fails with E0277.
- `anyhow::Error` does not implement `std::error::Error`. It derefs to
  `dyn Error + Send + Sync + 'static` and converts into `Box<dyn Error + Send + Sync>`.
- `err.downcast_ref::<E>()` on an `anyhow::Error` sees through `.context()` layers, but not
  through a thiserror `#[source]` field. Search the chain:
  `err.chain().find_map(|c| c.downcast_ref::<E>())`.
- `anyhow::Error::from_boxed(boxed)` and `anyhow!(boxed)` keep the box: a later
  `downcast_ref::<io::Error>()` returns `None`, and so does a chain search. Downcast the box
  before you wrap it.
- `fn main() -> Result<(), E>` prints `Error: {err:?}` and exits with status 1. For anyhow that
  is the `Caused by:` list, plus a backtrace when `RUST_BACKTRACE=1` is set.

## Triage

| Symptom | Cause | Fix |
| --- | --- | --- |
| `` E0277: `?` couldn't convert the error to `AppError` `` with `note: 'AppError' needs to implement 'From<io::Error>'` | No `From` impl for the source error | Add `#[from]` to a variant, or `map_err` into a variant with `#[source]` |
| The same E0277 with `help: for that trait implementation, expected 'B', found 'ParseIntError'` | A two-hop conversion | Convert the first hop: `.map_err(B::from)?` |
| `` `?` couldn't convert the error: `String: std::error::Error` is not satisfied `` | `?` into `anyhow::Error` from `String` | Return a real error type; at the call site, `.map_err(anyhow::Error::msg)?`. The std boxed error types accept `String` directly |
| `` `?` couldn't convert the error: `dyn std::error::Error + Send + Sync: Sized` is not satisfied `` | `?` from a `Box<dyn Error + Send + Sync>` into `anyhow::Error` | `.map_err(anyhow::Error::from_boxed)?`, or return a concrete error type |
| `` E0599: no method named `context` found for enum `Result<T, E>` `` | The `anyhow::Context` trait is not in scope | `use anyhow::Context as _;` |
| `` E0599: the method `context` exists for enum `Result<u8, String>`, but its trait bounds were not satisfied `` | The error type does not implement `std::error::Error` | `.map_err(anyhow::Error::msg).context(..)` |
| `` E0277: the trait bound `anyhow::Error: std::error::Error` is not satisfied `` | A generic `E: Error` parameter receives an `anyhow::Error` | Take `&(dyn Error + 'static)` and pass `err.as_ref()` |
| `` E0277: `dyn std::error::Error` cannot be sent between threads safely `` | `Box<dyn Error>` crosses a thread or task | Use `Box<dyn Error + Send + Sync + 'static>` |
| E0282 `type annotations needed` on `?` in a closure or an `async` block | Nothing names the error type of `?` | End with `Ok::<_, AppError>(value)` |
| `` the `?` operator can only be used on `Result`s, not `Option`s `` | `?` on an `Option` in a `Result` function | `.ok_or(Error::Missing)?`, or `.context("..")?` with anyhow |
| `` E0119: conflicting implementations of trait `From<std::io::Error>` `` | Two variants have `#[from]` of one type | Keep `#[from]` on one; `map_err` into the other |
| `` E0599: the method `as_dyn_error` exists for ... `` from a thiserror derive | A `#[source]` or `#[from]` field, or a field named `source`, is not an error | Rename the field, or wrap the value in an error type |
| `clippy::result_large_err` | The `Err` type is 128 bytes or more | Box the large variant's payload; set `large-error-threshold` in `clippy.toml` only with a reason |

## Choose the error type

| Code | Default | Reason |
| --- | --- | --- |
| Library public API | A thiserror enum or struct | The caller can match the cases it handles |
| Application, binary, test helper | `anyhow::Result` with `.context()` | The program reports the error; it does not match on it |
| Library callback or plugin result whose type the library cannot know | `Box<dyn Error + Send + Sync + 'static>` as a `#[source]` field | Keeps the chain, and `downcast_ref` reaches the concrete type |

Never put `anyhow::Error` or a bare `Box<dyn Error>` in the public signature of a library. The
caller can then only print the message or guess a type to downcast to.

## Design a library error

- **Granularity.** Give each variant one failure that a caller handles differently. Default:
  one enum per module or per operation family, so a function returns only variants it can
  produce. Use one enum per crate when the crate is small or its operations share most failures.
  Do not build a crate-wide enum in which each function produces 2 of 15 variants.
- **Context lives in fields.** Put the data a reader needs (path, key, id) in the variant, and
  the cause in a `#[source]` field. thiserror 2.0.21 formats a `PathBuf` field with `{path}`.
- **Display.** Write a short lowercase message with no trailing period, and leave the source out
  of it. This is the std convention and a catalog default.
- **Thread-safe.** Keep every error `Send + Sync + 'static`. Assert it once per public type.
- **`#[non_exhaustive]`** on every public error enum from its first release. The
  `rust-discipline` skill owns that rule.
- **Dependency types.** A `#[from] dep::Error` field or a `pub` `impl From<dep::Error>` makes
  the dependency's major version part of your API. Read
  [`references/library-errors.md`](references/library-errors.md) when a public error holds or
  converts a third-party error, when you want an opaque error with a `kind()`, or when you add
  error codes.

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("cannot read the config file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid port {value:?}")]
    Port {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
}

// Compile-time check that the error can cross threads and enter anyhow.
const _: () = {
    const fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}
    assert_error::<ConfigError>();
};

pub fn load_port(path: &Path) -> Result<u16, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|source| ConfigError::Read { path: path.to_path_buf(), source })?;
    let value = text.trim();
    value.parse().map_err(|source| ConfigError::Port { value: value.to_owned(), source })
}
```

## Propagate and add context

`?` converts the error with `From::from`. When no impl exists, the build fails:

```rust,compile_fail,E0277
#[derive(Debug)]
pub struct AppError;

pub fn read() -> Result<String, AppError> {
    // `?` couldn't convert the error to `AppError`
    let text = std::fs::read_to_string("app.toml")?;
    Ok(text)
}
```

In an application, add context at each layer with `anyhow::Context`. `.context()` takes a
value; `.with_context(|| ..)` formats only on the error path. Both keep the source, and both
also work on an `Option`.

```rust
use anyhow::Context as _;
use std::path::Path;

pub fn port(path: &Path) -> anyhow::Result<u16> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let line = text.lines().next().context("the file is empty")?;
    line.trim().parse().with_context(|| format!("invalid port {line:?}"))
}

pub fn is_missing(err: &anyhow::Error) -> bool {
    // The context layer keeps the source, so downcast reaches the io::Error.
    err.downcast_ref::<std::io::Error>()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
}
```

`?` converts every `E: Error + Send + Sync + 'static`, a `&str`, and a `String` into
`Box<dyn Error + Send + Sync + 'static>`. Keep `'static`: `downcast_ref` needs it.

## Report the error

- Report once, at the boundary that decides what happens next: `main`, a request handler, a
  task supervisor. Default: a layer either returns the error or logs it, not both, because a
  log-and-return prints one failure several times.
- For a person, print the whole chain on one line: `{err:#}` for `anyhow::Error`, or a loop over
  `source()` for any other error. `{err}` prints only the outer message.
- For a machine, expose a stable code, not the message. Match on a variant or a `code()`
  string, never on `Display` text, which a wording fix changes.
- In `main`, return `std::process::ExitCode`, or a type that implements
  `std::process::Termination`. The `rust-cli` skill owns which status means what.
  `std::error::Report` is unstable as of Rust 1.99.1 (`error_reporter`).

Read [`references/main-and-logs.md`](references/main-and-logs.md) when you write `main`, map an error
to an exit status, print a chain without anyhow, or log an error.

## Verify

| Claim | Check | What a green result does not prove |
| --- | --- | --- |
| Every `?` converts | `cargo check --all-targets --locked` | That the conversion kept the source |
| The source chain is complete | A test that walks `source()` and asserts each layer's message, or `format!("{err:#}")` for anyhow | Other error paths than the one the test triggers |
| No message repeats its source | The same chain test: each cause appears once | |
| The error is `Send + Sync + 'static` | The `const` assertion above compiles | Nothing about errors inside a `Box<dyn Error>` field |
| A public error change keeps SemVer | `cargo semver-checks check-release`, if it is installed | A dependency type in a public field or `From` impl; review it by hand |
| The `Err` type stays small | `cargo clippy --all-targets --locked -- -D warnings` (`result_large_err`) | Size on the success path |

# Print the chain, return a status from main, and log an error

Facts are for anyhow 1.0.104 on Rust 1.99.1.

## Print the chain

| Form | `anyhow::Error` prints | Use it for |
| --- | --- | --- |
| `{err}` | The outer message only: `load settings.toml` | Never alone for a person: the cause is lost |
| `{err:#}` | The chain on one line: `load settings.toml: no such file` | A person, a one-line log record |
| `{err:?}` | The outer message, a `Caused by:` list, and a backtrace when one was captured | A developer, a crash report |

anyhow captures a backtrace when `RUST_LIB_BACKTRACE=1` is set, or when `RUST_BACKTRACE=1` is
set and `RUST_LIB_BACKTRACE` is not `0`. Do not show a backtrace to a user by default.

Any other error has no `{:#}` chain. Walk `source()` yourself. `std::error::Report` does this,
but it is unstable as of Rust 1.99.1.

```rust,run
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

fn chain(err: &(dyn Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut next = err.source();
    while let Some(cause) = next {
        let _ = write!(out, ": {cause}");
        next = cause.source();
    }
    out
}

#[derive(Debug)]
struct LoadError {
    source: std::io::Error,
}

impl fmt::Display for LoadError {
    // The cause stays out of the message: `chain` prints it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cannot load settings")
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

fn main() {
    let err = LoadError { source: std::io::Error::other("disk offline") };
    assert_eq!(chain(&err), "cannot load settings: disk offline");
}
```

Pass an `anyhow::Error` to this function as `err.as_ref()`. A generic `E: Error` parameter
rejects it with E0277, because `anyhow::Error` does not implement `Error`.

## Return a status from `main`

`fn main() -> Result<(), E>` prints `Error: ` and the `Debug` form to stderr, then exits with
status 1. Measured with anyhow:

```text
Error: cannot load settings

Caused by:
    No such file or directory (os error 2)
```

That output fits a developer tool. For a user-facing program, return
`std::process::ExitCode`, print `{err:#}` to stderr, and pick the status from the error. The
`rust-cli` skill owns the status table and the stderr rules.

Search the chain when you pick the status. `anyhow::Error::downcast_ref` sees through
`.context()` layers, but not through a thiserror `#[source]` field; `chain()` sees both.

```rust
use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration")]
pub struct ConfigError;

fn run() -> anyhow::Result<()> {
    todo!()
}

fn status(err: &anyhow::Error) -> ExitCode {
    if err.chain().any(|cause| cause.is::<ConfigError>()) {
        ExitCode::from(2)
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            status(&err)
        }
    }
}
```

To keep this logic out of `main`, implement `std::process::Termination` on a report type and
return it from `main`:

```rust
use std::process::{ExitCode, Termination};

pub struct Report(pub anyhow::Result<()>);

impl Termination for Report {
    fn report(self) -> ExitCode {
        match self.0 {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::FAILURE
            }
        }
    }
}

fn main() -> Report {
    Report(Ok(()))
}
```

Do not call `std::process::exit` from a nested function. It runs no destructors, so buffered
output and temporary files stay behind.

## Log an error

- Log each failure once, at the layer that handles it. A layer that logs and then returns the
  error makes one failure appear once per layer. This is a catalog default.
- Put the whole chain in the record: `{err:#}` for anyhow, or the `chain` function above.
  `{err}` alone loses the cause.
- Keep secrets out of messages and context strings. A context string such as
  `format!("cannot connect to {url}")` copies a token in the URL into every log line.
- The `rust-observability` skill owns log fields and subscribers. Without it, record the chain
  as one string field, `format!("{err:#}")`.

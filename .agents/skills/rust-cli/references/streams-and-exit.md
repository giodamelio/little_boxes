# Output streams and exit status

Read this when you write the stdout path of a command, map a broken pipe, or map a parser error
or failure to an exit status.

## Broken pipe and the drop-time flush

A Rust binary starts with `SIGPIPE` ignored on Unix, so a write to a closed pipe returns
`ErrorKind::BrokenPipe` and does not kill the process. `println!` panics on that error. Lock
stdout and propagate write errors instead. Call `flush()` on a `BufWriter` before it drops: drop
ignores a flush error, so the last buffered rows can disappear while the command exits with
status 0.

```rust,run
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

fn write_rows(out: impl Write, rows: &[&str]) -> io::Result<()> {
    let mut out = BufWriter::new(out);
    for row in rows {
        writeln!(out, "{row}")?;
    }
    out.flush()
}

fn exit_status(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        // The consumer closed stdout early, as in `tool list | head -n 1`.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tool: cannot write output: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    // Probe: the row fits in the buffer, so only flush() reaches the closed pipe.
    let (reader, writer) = io::pipe().expect("create pipe");
    drop(reader);
    let error = write_rows(writer, &["one"]).expect_err("the pipe is closed");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);

    exit_status(write_rows(io::stdout().lock(), &["one", "two"]))
}
```

Do not translate every broken pipe to success. A broken pipe on an upload, socket, log sink, or
required output file is an operation failure. On Unix, do not globally restore the default
`SIGPIPE` disposition without reviewing all writes in the process. A Rust library loaded by a
non-Rust host does not get the ignored `SIGPIPE`; use the `rust-debugging` skill, when it is
installed, for that case. On Windows, handle the corresponding I/O error instead of assuming a
POSIX signal.

## Exit codes

Keep portable application codes in `0..=125`. Shells commonly reserve `126`, `127`, and
`128 + signal`; Windows does not give those values the same process model.

`std::process::exit` runs no destructors, so a live `BufWriter`, temporary-file guard, or lock
guard loses buffered output and cleanup. A `main` that returns `Result<(), E>` makes std print
the `Debug` form, such as `Error: Custom { kind: NotFound, error: "config.toml missing" }`, and
exit with status 1.

As of clap 4.6, `Error::exit()` prints help and version to stdout with status 0 and a usage error
to stderr with status 2, then calls `std::process::exit`. `Cli::parse()` calls `Error::exit()`,
so parse before you create a buffer or guard. For a clap error raised after work starts, call
`let _ = err.print();` and return `ExitCode::from(u8::try_from(err.exit_code()).unwrap_or(2))`.
`ExitCode` has no `From<i32>`, and `exit_code()` returns `i32`. Preserve these statuses unless
the CLI has a documented older contract. Do not catch a parser error and turn every result into
status `1`.

## The `clap` definition test

`#[test] fn verify_cli() { Cli::command().debug_assert(); }`, with `clap::CommandFactory` in
scope, reports a definition error such as a duplicate short flag. Without it, clap reports the
error as a debug assertion only on a path that reaches it. Put the test in the crate that defines
`Cli`, such as the binary's `#[cfg(test)] mod tests`, because an integration test cannot import a
binary crate's items. As of clap 4.6, these asserts compile only with `debug_assertions`, so the
test proves nothing under `cargo test --release`.

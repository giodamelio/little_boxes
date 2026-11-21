# Backtraces and host logging

Deep material for `rust-debugging`, section 1. Read it when you tune backtrace variables or
install a host subscriber.

## Backtrace variables

- `RUST_LIB_BACKTRACE` wins over `RUST_BACKTRACE` for `std::backtrace::Backtrace::capture`, which
  `anyhow` calls when it creates an error. With `RUST_BACKTRACE=1` alone, every such error pays
  for a capture. Set the variables before the process starts; std caches them at the first capture.
- v0 mangling is the default since 1.97. `RUST_BACKTRACE=full` frames now carry crate hashes, for
  example `std[5d97c59e5e5fafcc]::panicking::default_hook`. The hash changes with the toolchain and
  the build configuration, so diff `RUST_BACKTRACE=1` output, not `full` output.
- Since 1.91 the panic line carries the thread ID: `thread 'main' (6583488) panicked at
  src/lib.rs:4:23:`. Grep for `panicked at`, not for `thread 'main' panicked`.

## Host subscriber with span timing

For a host repro, install a subscriber that logs span timing. `EnvFilter` needs the `env-filter`
feature of `tracing-subscriber`.

```rust
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

fn main() {
    // Host repro only. On a device, the application bootstrap owns the subscriber.
    // The default writer is stdout; stderr keeps the program's stdout clean.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();
}
```

```bash
RUST_LOG=my_crate=debug,my_other_crate=trace cargo run --locked -p my-cli -- <args>
```

- Without `with_span_events`, the `fmt` subscriber logs no span lifecycle. `#[instrument]` then
  only adds context to events.
- `FmtSpan::CLOSE` logs one event per closed span with `time.busy` and `time.idle`. High idle time
  means the span waited. High busy time means it did the work.
- Instrument a suspect function with `#[instrument(skip_all, fields(...))]`. The
  `rust-observability` skill owns the field rules and the device subscriber setup.

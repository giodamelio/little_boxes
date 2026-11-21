# Dispatcher Install and the `log` Bridge

This reference expands the dispatcher rules in `SKILL.md`, and says what a host,
a bootstrap `cdylib`, and an embedded sink each see. The six rules, the gates,
and the "It emits nothing" triage table stay in `SKILL.md`.

Contents:

- Why one install
- The sink registry
- Keep callsite interest valid
- Bridge `log` once
- Keep the `WorkerGuard`
- Test the install
- Host, bootstrap, and embedded sink (and an Android bootstrap sink)

## Why one install

An application can load two FFI crates from the same workspace. If each one
installs a subscriber, the second install returns an error that an init
function usually discards. That boundary then stays silent for the life of the
process.

Put the dispatcher and the sink registry in one shared observability crate.
Install the dispatcher once. Let every boundary add its sink to the registry
owned by that installed subscriber.

## The sink registry

```text
install_dispatcher() -> Result<(), InstallError>         process bootstrap, once
register_sink(id, sink, level) -> Result<(), SinkError>   shared sink registry
init_core_logging(sink, level)                           registers sink "core"
init_render_logging(sink, level)                         registers sink "render"
```

`install_dispatcher` is the only function that calls `set_global_default`. Store
its result in a one-time state. Do not retry a failed global install from a
boundary. `register_sink` only mutates the installed subscriber's fan-out
registry. Reject a duplicate sink ID, and report a missing or failed dispatcher
separately from a duplicate registration.

Logging remains optional for application start-up. Return installation and
registration status to the host. Do not panic, and do not turn a diagnostic
failure into a domain-operation failure.

## Keep callsite interest valid

Keep callsite interest valid when the sink set can change. Make the fan-out
subscriber's `register_callsite` return `Interest::sometimes()` for every
callsite. Do not return `Interest::never()` only because the registry is empty.
The callsite caches that result, so a sink registered later cannot receive that
callsite. If the subscriber instead caches interest from the current sinks,
release the registry lock and call
`tracing_core::callsite::rebuild_interest_cache()` after every sink, level, or
filter mutation. Rebuild after a change to `max_level_hint` too.

## Bridge `log` once

The `log` crate allows one logger per process. Each of these installs one:

- `android_logger::init_once`. When another logger is already set, it fails
  silently.
- `tracing_log::LogTracer::init()`. It returns `SetLoggerError`: "attempted to
  set a logger after the logging system was already initialized".
- `SubscriberInitExt::try_init()` and `init()`, and the `fmt` builder's
  `tracing_subscriber::fmt()...try_init()` and `init()`, while the default
  `tracing-log` feature of `tracing-subscriber` is on.
  `SubscriberInitExt::init()` panics with "failed to set global default
  subscriber". The `fmt` builder's `init()` panics with "Unable to install
  global subscriber".

Route `log` records into the dispatcher through `LogTracer`, and do not call
`android_logger::init_once`. The other direction, `android_logger` as the one
`log` logger, writes dependency records to logcat past the redacting visitor.

In `install_dispatcher`, call `tracing::subscriber::set_global_default`, then
`tracing_log::LogTracer::init()`. Report the two results as separate
`InstallError` cases: when only the bridge fails, the dispatcher still works.
Do not use `try_init()` there. It sets `log::max_level()` from the dispatcher's
level ceiling at install time. With no sink registered, that ceiling is off, so
`log::max_level()` stays `Off`, and the `log` macros drop every dependency
record before the bridge sees it. `LogTracer::init()` sets `log::max_level()` to
`Trace`, and the bridge checks the current `tracing` ceiling for each record.
Use `try_init()` only in a host binary whose filter is fixed at install.

Never call `init()` from `JNI_OnLoad` or an FFI init export. It panics when a
global subscriber or `log` logger is already set. In an unguarded export, or
under `panic = "abort"`, the panic aborts the process (Rust 1.81 and later).
Under a `catch_unwind` guard, the load fails with `JNI_ERR`, and the panic
message goes to stderr, which an Android app discards.

## Keep the `WorkerGuard`

Bind a `tracing_appender::non_blocking` `WorkerGuard` in `main` for the life of the process:
when it drops, the writer thread stops and later events are lost.

## Test the install

Cover this with one process-level test. Put it alone in its own integration-test
file: the global install succeeds once per process, and `cargo test` runs all
tests of one file in one process. cargo-nextest, when installed, runs each test
in its own process.

1. Install once with no sinks. Call one `emit_probe` helper and assert that no
   record arrives.
2. Register a sink. Call the same helper again and assert that exactly one
   record arrives. The same static callsite catches a stale cached
   `Interest::never()`.
3. Register the other boundary sink. Emit through both boundaries and assert
   that both sinks receive their records.
4. Emit one `log::info!` record and assert that it reaches the sink. This proves
   that exactly one `log` logger is installed, that it is the bridge, and that
   the install did not leave `log::max_level()` at `Off`.
5. Assert that a duplicate ID is rejected without replacing the original sink.
6. Emit one event with a message, a `%` value, and an undeclared field. Assert
   that the sink record holds only the declared fields and no message.

## Host, bootstrap, and embedded sink

The six rules and the gates in `SKILL.md` apply to all three; each sees a
different part of an event.

- **Host** (a CLI, a service, or a test binary): it owns rule 1's single
  install and may call `init()`. Rules 5 and 6 apply as written. Rules 2 to 4
  apply to each emission that leaves the process (an exporter, a log store, a
  crash reporter). A `tracing-subscriber` formatting layer renders the full
  event, including an error's `Display` message. Write to stderr so stdout
  stays machine-readable. In a CLI, keep the layer off unless an environment
  filter is set. The layer renders untrusted text: use `tracing-subscriber`
  0.3.20 or later, which escapes ANSI sequences in logged values
  (RUSTSEC-2025-0055), and gate advisories with
  `cargo deny --config deny.toml --locked check advisories`, or
  `cargo audit --deny warnings` without cargo-deny.

  ```bash
  RUST_LOG=my_pipeline=debug my-cli render input.toml
  ```

- **Bootstrap `cdylib`** (the application-owned outermost library that a mobile
  app or plugin host loads): it owns the install and may depend on
  `tracing-subscriber`. In a shipped build, its platform sink receives the
  redacted record, like any embedded sink. The Android case follows.

- **Embedded sink** (a callback registered across FFI): receives a redacted
  record: severity, target, callsite name, and the fields that survive the
  visitor. **Never the message.** An error arrives as its kind, a frozen
  enumeration case, and nothing else.

Only a host binary and the bootstrap `cdylib` depend on `tracing-subscriber`.
Engine crates, the shared observability crate, and inner FFI library crates do
not. Put the layer in the host instead.

### An Android bootstrap sink

An Android app process sends stderr to `/dev/null` and does not inherit a shell
environment, so stderr and `RUST_LOG` do not apply. `tracing-logcat` is a `fmt`
`MakeWriter`: it writes the rendered event, message and `%`/`?` values
included. Add it only in a debug or opt-in build where the privacy floor
permits full text. `tracing-android` has had no release since 2022.

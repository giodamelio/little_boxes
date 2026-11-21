---
description: Use when adding or reviewing tracing, logging, or production metrics in Rust, especially in a library or cdylib behind FFI that exposes a telemetry snapshot to its host. Also for metric naming, histogram boundaries, label cardinality, OpenTelemetry context propagation, and exporter shutdown. Triggers on tracing-subscriber, set_global_default, LogTracer, SetLoggerError, EnvFilter, instrument skip_all, opentelemetry_sdk.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-observability
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 567a67b7ae5a2a2e8507a5e44285dd1d84aea398
name: rust-observability
---
# Rust Observability

These rules assume a Rust library that a host embeds: a cdylib behind FFI, a
staticlib, or a crate that a host CLI or service links. The host owns the
process; the library owns only its emissions. For a service or CLI binary, read
[references/dispatcher-install.md](references/dispatcher-install.md#host-bootstrap-and-embedded-sink)
to see which rules apply.

## The six rules that are not style

1. **One dispatcher install per process.** Only the host (the binary, or the
   application-owned outermost bootstrap `cdylib`) calls
   `tracing::subscriber::set_global_default`, once, at process bootstrap. Its
   subscriber owns a fan-out registry for all embedded sinks. Do not install
   once per sink or per FFI boundary: the second install fails, and that
   boundary stays silent.
2. **`skip_all` plus explicit `fields(...)` on every `#[instrument]`.** Without
   `skip_all` the macro records every argument through `Debug`. Do not add `err`
   or `ret` on a path that can reach a sink you do not control: they record the
   error through `Display` and the return value through `Debug`.
3. **No `?` and no `%` sigils, and no `format!` inside an emission** that can
   reach a sink you do not control. All three produce free text.
4. **Field names come from one declared vocabulary.**
5. **Nothing on the data plane emits an event.** Data-plane work increments an
   atomic counter or pushes into a bounded queue, except an `error!` or `warn!`
   on an error path.
6. **Diagnostics are observational.** No code path reads the outcome of an
   emission. Turning a subscriber on must not change a single output byte.

The `cargo tree` gates below and the install test prove rule 1; rules 2 to 4
need the repository gate; the hot-path search finds rule 5 candidates; the
determinism test proves rule 6. No gate proves that a field value is safe.

## Gates and what they prove

Run these before you push a change to an emission, once for each engine crate
and inner FFI crate.

```bash
set -e
cargo test --locked -p <observability-crate>
cargo clippy --locked --workspace --all-targets -- -D warnings
deps="$(cargo tree --locked -p <library-crate> -e normal --target all --prefix none)"
if grep -q '^tracing-subscriber ' <<<"$deps"; then echo 'FAIL: library links tracing-subscriber'; exit 1; fi
features="$(cargo tree --locked -p <library-crate> -e features --target all --prefix none)"
if grep -qE '^(tracing|log) feature "(release_)?max_level_' <<<"$features"; then echo 'FAIL: max_level_* under a library'; exit 1; fi
```

Do not rewrite them as `cargo tree -i`, `! grep`, or a pipe into `grep -q`:
each passes a broken library ([references/gates.md](references/gates.md) says why).

Run the hot-path search on each crate that does per-item or per-byte work:

```bash
rg -n --type rust \
  '\b(trace|debug|info|event|span|(trace|debug|info|warn|error)_span)!|#\[(tracing::)?instrument' \
  <hot-path-crate>/src
```

Add a repository gate that reads the vocabulary tables and rejects breaks of
rules 2 to 4, with a `--self-test` mode ([references/gates.md](references/gates.md)). Deny `clippy::print_stdout`, `clippy::print_stderr`, and `clippy::dbg_macro` in
library, FFI, and embedded crates: stdio bypasses the redacting visitor and
reaches no reader on a mobile target ([references/gates.md](references/gates.md) has the configuration).

## Done when

The gates are green. Then review what they cannot see:

- [ ] Each field value is safe, not only its name. No correlation identifier
      is derived from a forbidden value.
- [ ] The instrumented entry point is the one the boundary actually calls.
- [ ] One bootstrap path installs: `set_global_default`, then
      `LogTracer::init()`. A library never calls `init()` or `try_init()`: in
      `JNI_OnLoad` or an FFI init export it panics on a second install.
- [ ] An embedded sink record holds only declared fields, no message. The
      queue is bounded, drops the oldest, and counts the drop in the snapshot.
- [ ] The determinism test passes: one operation runs twice in one process,
      with no subscriber and with the most verbose one, and the outputs
      byte-compare. A negative control mutates one input and must fail.
- [ ] A panic hook emits only bounded structured fields, never raw
      `PanicHookInfo`, its payload, or a backtrace.

## "It emits nothing"

| Check | Symptom when it is the cause |
|-------|------------------------------|
| Is a sink registered at all? | With no sink the level ceiling is off, and `enabled` rejects every callsite. |
| Was this callsite emitted before the first sink registered? | The subscriber cached `Interest::never()`. Return `Interest::sometimes()` for a mutable registry, or rebuild the interest cache after each mutation. |
| Did the one-time install fail? | Another global subscriber won ("a global default trace dispatcher has already been set"). Do not retry from each boundary. |
| Do `log` records from dependencies vanish? | A second `log` logger (for example `android_logger`) was installed first, so `LogTracer` got `SetLoggerError`. Keep one bridge. |
| Do `log` records never arrive, even after a sink registers? | `try_init()` set `log::max_level()` to `Off` when no sink existed. Install with `set_global_default` plus `LogTracer::init()`. |
| Did sink registration fail? | The dispatcher is unavailable or the sink ID is taken. |
| Is the sink's level above the emission's level? | Only higher-severity events arrive. |
| Does a dependency enable a `max_level_*` feature? | The callsites are compiled out. `cargo tree -e features -i tracing --workspace` (or `-i log`) names the crate. |
| On a host, is `RUST_LOG` valid? | `EnvFilter::from_default_env` prints `ignoring ...`, drops each invalid directive, and falls back to `error`; `try_from_default_env` returns `Err`. |
| Is the instrumented function on the path you exercise? | Unit tests pass, the CLI prints nothing (step 3 of *Add an emission*). |
| Is the emission in a `#[cfg(test)]` module? | The release build drops it. |
| Is the crate target in the filter? | `RUST_LOG=debug` is not `RUST_LOG=my_crate=debug` when a dependency floods the output. |

## Privacy floor

Diagnostic emission and every telemetry snapshot must not carry:

- Device, network, hardware, or subscriber identifiers, raw or hashed under any
  encoding: a salted hash still supports correlation and dictionary attacks.
  Emit only aggregate counts or a local non-identifying category.
- Addresses that identify a user's device or its location.
- Secrets, key material, or handshake payload bytes.
- Message or packet payloads. Counters and sizes are allowed. An opaque flow
  identifier must be random, short-lived, scoped to one process or session, and
  never derived from a forbidden value.
- User data in a panic, `expect`, or `unreachable!` message: a panic message is
  a log line that can reach a crash report.

Audit before a release. The grep is a floor, not a proof: two leaks no regex
finds are in the *smuggling routes* of
[references/redaction-and-field-vocabulary.md](references/redaction-and-field-vocabulary.md).

```bash
rg -i 'bssid|ssid|imei|imsi|raw_ip|latitude|longitude' \
  . --type rust -n | grep -v '// allow:'
```

## Add an emission

```rust
#[tracing::instrument(skip_all, fields(stage = stage.code()))]
fn decode(stage: DecodeStage, items: &[Item]) -> Result<Output, EngineError> {
    tracing::debug!(item_count = items.len(), "decoding");
    todo!()
}
```

1. Record a closed code, not the argument, as `fields(stage = stage.code())` does.
2. Take each field name from the vocabulary tables, and judge the value it
   carries: a `count` field that holds a coordinate is a leak.
3. **Instrument the entry point that the boundary calls.** An FFI entry and a
   CLI entry often do the same work. Instrumenting the wrong one compiles,
   passes unit tests, and emits nothing. Exercise the path with the host tool.
4. A span or an event macro can push a frame past a small
   `clippy::large_stack_frames` threshold ([references/gates.md](references/gates.md)).

Read [references/redaction-and-field-vocabulary.md](references/redaction-and-field-vocabulary.md)
when you add a field name, write a sink visitor, or review an emission for
leakage.

## Severity

| Level | Use for |
|-------|---------|
| `error` | The operation failed. |
| `warn` | Recoverable degradation. The operation continues. |
| `info` | A boundary event: start, stop, configuration applied. |
| `debug` | Detail for one operation. |
| `trace` | Per-item detail in a control-plane loop. Never on the data plane. |

Cancellation is `debug`: the caller got what it asked for. No tracing level is
off in a release build by default: configure and test the release filter, and
do not rely on `cfg(debug_assertions)` to remove callsites. Do not enable the
`max_level_*` or `release_max_level_*` features of `tracing` or `log` in a
library: Cargo unifies features, so one library's choice compiles out callsites
in every crate of the binary.

## One dispatcher install, multiple sinks

Put the dispatcher and the sink registry in one shared observability crate.
`install_dispatcher` alone calls `set_global_default`; each FFI boundary only
calls `register_sink`. Store the install result in a one-time state, and report
the `set_global_default` and `LogTracer::init()` results separately to the host.
Do not panic, and do not turn a diagnostic failure into a domain failure. Test
the install alone in its own integration-test file.

Read [references/dispatcher-install.md](references/dispatcher-install.md) when
you write or change `install_dispatcher`, `register_sink`, the fan-out
subscriber, the `log` bridge (`android_logger`, `try_init()`), a
`WorkerGuard`, the install test, or an Android bootstrap sink.

## Host, bootstrap, and embedded sink

Only a host binary and the bootstrap `cdylib` depend on `tracing-subscriber`.
An embedded sink receives a redacted record and **never the message**. The
host-and-sink section of the dispatcher reference says what each one sees.

## Counters, the bounded queue, and the snapshot

`tracing` serves the control plane: lifecycle, configuration, errors. A native
trace backend, for example Perfetto, stays behind a debug-only feature. On the
data plane, count with `AtomicU64::fetch_add(n, Ordering::Relaxed)`, queue into a bounded
drop-oldest queue that counts its drops, and let the host poll one snapshot; no
FFI call per item. Read [references/embedded-telemetry-surface.md](references/embedded-telemetry-surface.md)
when you choose a channel for a new path, or change a counter, the queue, the
snapshot, or readiness.

## Production metrics and distributed context

Never use an identifier, free-text error, URL, or trace ID as a metric label.
Do not hold a
`Span::enter()` guard or a thread-local context guard across `.await`: the next
task polled on that thread inherits the span. Use `.instrument(span)` or
`#[instrument]`. An export failure never changes the domain result. Read
[references/production-metrics-and-propagation.md](references/production-metrics-and-propagation.md)
when you add a production metric, propagate OpenTelemetry context, change an
exporter, or upgrade the `opentelemetry` crates.

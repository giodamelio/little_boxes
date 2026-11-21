# The Embedded Telemetry Surface

This reference expands the channel, queue, and snapshot sections in `SKILL.md`,
and adds snapshot golden tests, the telemetry side of panic reporting, and stall
detection. It applies when a Rust library runs inside a host process — a mobile
application, a game engine, a daemon plugin — and the host reads telemetry
across a boundary.

Contents:

- Why the shape differs from a server
- Channel costs
- Counters and the snapshot boundary
- The bounded event ring: contract, code, and what not to use
- The snapshot entry point, and readiness as the exception
- Golden-test the snapshot: key order and scrub paths
- Panic reporting: the telemetry side
- Stall detection: heartbeat and Tokio runtime metrics

## Why the shape differs from a server

- A boundary crossing per event is a measurable cost (see *Channel costs*). At a
  million items per second that is the whole budget.
- Several consumers want the same stream: a UI, a golden-test harness, an
  exporter. The channel choice decides whether a slow consumer blocks the
  producer.
- The host can kill the process at any moment. A low-memory killer needs no
  warning. Any "exactly once" delivery assumption is wrong. Design for loss and
  report the loss.
- The privacy floor in `SKILL.md` constrains what may be emitted at all, before
  performance is even a question. Everything in the snapshot crosses the
  boundary and often ends up in a bug report.

## Channel costs

| Channel | Use for | Forbidden for | Cost |
|---------|---------|---------------|------|
| `tracing` events and spans through the installed subscriber | Control plane: lifecycle, configuration, errors, single-shot diagnostics, control-flow spans | Per-packet, per-byte, per-item paths | Formatting plus an atomic load on the callsite cache; roughly a few microseconds per event when a platform log writer is behind it |
| The `log` crate, bridged into the same subscriber | Dependencies that only speak `log` | The same hot paths | Roughly 1 µs per event with no arguments, roughly 3 µs with formatted arguments, when a syscall or FFI call (for example a JNI call into the Android logger) is behind it |
| A native trace backend, for example Perfetto through `tracing_android_trace` | Performance investigation only | Anything enabled by default in release | Heavy. Put it behind a debug-only feature flag |
| `AtomicU64::fetch_add` | Data-plane counters: items, bytes, drops, errors | Anything that is not a count | An `ldxr`/`stxr` loop on the default `aarch64-linux-android` and `aarch64-apple-ios` targets; one `ldadd` where LSE is on (`aarch64-apple-darwin`, or `-C target-feature=+lse`). rustc 1.99.1, opt-level 3 (the `aarch64-apple-darwin` case on 1.98.1) |
| A bounded queue drained by a poller | Data-plane events that a consumer must see individually | Anything that must never be dropped | One bounded push, no allocation on the steady path |

Re-measure the microsecond figures on your own target. Treat them as orders of
magnitude, not as constants.

## Counters and the snapshot boundary

Count data-plane work with `AtomicU64::fetch_add(n, Ordering::Relaxed)`. A
counter needs atomicity, not a happens-before edge. Two relaxed counters read
in one pass are not consistent with each other, so do not derive an invariant
from their ratio.

The host polls one serialized snapshot that carries the counters and the
drained queue. Do not call across the FFI boundary per item or per telemetry
event: the per-call cost is the whole budget on a hot path. A one-shot
readiness callback is the one exception.

## The bounded event ring

One ring per domain. Each ring owns an atomic bounded queue and a drop counter.

Contract. Preserve every point if you change the implementation.

1. The ring is **bounded**. Pick the capacity per domain.
2. On a full ring, **atomically replace the oldest record**. The newest records
   are the ones you need after a fault.
3. Every eviction increments a **dropped-event counter** that the snapshot
   exposes.
4. Retained records keep **FIFO order**.
5. `push` takes no mutex, does not deliberately sleep, and does not allocate on
   the steady path. It can spin while a preempted peer owns a slot, so it has no
   bounded latency and no formal lock-free progress guarantee. Do not call it
   from an interrupt, a reentrant signal handler, or another real-time context.

```rust
use core::sync::atomic::{AtomicU64, Ordering};
use crossbeam_queue::ArrayQueue;

pub struct Record;

pub struct EventRing {
    queue: ArrayQueue<Record>,
    dropped: AtomicU64,
}

impl EventRing {
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "event ring capacity must be non-zero");
        Self {
            queue: ArrayQueue::new(capacity),
            dropped: AtomicU64::new(0),
        }
    }

    /// Takes no mutex and never sleeps. On a full ring the oldest record is
    /// evicted. Call from task context, not an interrupt or signal handler.
    pub fn push(&self, record: Record) {
        // `force_push` atomically inserts the new record and returns the oldest
        // record only when the queue was full. A concurrent drain cannot split
        // eviction from insertion.
        if self.queue.force_push(record).is_some() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn drain(&self) -> Vec<Record> {
        // Bound one batch to the queue length observed at entry. Producers can
        // refill the queue while this call runs, but they cannot extend this
        // call beyond the entry budget.
        let available = self.queue.len();
        (0..available)
            .map_while(|_| self.queue.pop())
            .collect()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}
```

A receive-then-send sequence on an MPMC channel is not an atomic drop-oldest
operation. A concurrent drain can run between the two steps.

Normalize domain aliases at the boundary, so a caller that names a domain
slightly differently lands in the right ring instead of creating a new one.

### What not to use

| Alternative | Failure |
|-------------|---------|
| `tokio::sync::broadcast` | Fan-out semantics. Every subscriber gets every message and must handle `Lagged`. Consumers here drain one ring; they are not independent subscribers. |
| An unbounded channel | A slow consumer becomes an out-of-memory kill. |
| A blocking mutex-backed buffer | A slow consumer stalls the data plane. |
| A ring with drop-newest | The most recent events are the ones you need after a fault. |

## The snapshot entry point

The host polls. It does not receive pushes per event.

```rust
use jni::objects::{JObject, JString};
use jni::{EnvUnowned, jni_mangle};

// Exports Java_com_example_app_TelemetryNative_jniSnapshot.
#[jni_mangle("com.example.app.TelemetryNative")]
pub fn jni_snapshot<'local>(
    mut env: EnvUnowned<'local>,
    _thiz: JObject<'local>,
) -> JString<'local> {
    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        let snap = TelemetrySnapshot {
            counters: COUNTERS.read(),
            events: EVENT_RING.drain(),
            dropped_events: EVENT_RING.dropped(),
        };
        let Ok(json) = serde_json::to_string(&snap) else {
            env.throw("telemetry snapshot encoding failed")?;
            return Err(jni::errors::Error::JavaException);
        };
        env.new_string(json)
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
```

`resolve` is the exit: it rebuilds an `Env`, throws for an error or a caught
panic, and returns the default. The helper names differ per binding version.
In jni 0.22, `ThrowRuntimeExAndDefault` throws only when no exception is
pending, and it puts the error's `Display` text in the message. Throw a fixed
message before you return `Error::JavaException`, or Java gets "Rust error:
Java exception was thrown". For a panic, the policy copies a `&'static str`
payload into the message and replaces a formatted payload with a fixed string.
The `rust-jni` skill owns the binding rules; at minimum, propagate or clear every
`Err(JavaException)` before the next JNI call. The `uniffi-boundary` skill owns a
generated surface; there, derive `uniffi::Error` on the exported error type.

What matters, independent of binding:

- One call returns counters and drained events together.
- The drain empties the ring, so a slow poller loses only through the ring's own
  drop-oldest rule, and the drop count says how much.
- The serialized form is stable enough to golden-test. See *Golden-test the
  snapshot*.
- Nothing in the data plane calls into the host.

### Readiness is the exception

Polling for a readiness sentinel adds the whole poll interval to start-up
latency. Use a one-shot readiness callback instead.

Keep the readiness path separate from the snapshot path:

- Register the readiness callback with its own generation token, so a callback
  from a previous runtime instance is ignored.
- Fire it once per runtime generation.
- Do not put a readiness flag in the snapshot. If both exist, the two disagree
  during the window between them.

## Golden-test the snapshot

Goldens compare snapshots. Serialize them with `serde_json::to_string_pretty`
(two-space indent). Key order depends on the type:

- A derived `Serialize` struct writes fields in declaration order. That is
  stable.
- A `HashMap` iterates in a random order for each map. Use `BTreeMap` instead.
- `serde_json::Value` and `serde_json::Map` sort keys, unless the
  `preserve_order` feature is on anywhere in the build. Feature unification can
  turn it on from another crate, and then insertion order wins. The `rust-serde`
  skill owns the full `serde_json` map facts.

Scrub the volatile fields before the diff. Keep the list in one file next to the
goldens.

```json
{
  "scrub_paths": [
    "$.events[*].timestamp_ms",
    "$.events[*].request_id",
    "$.counters.uptime_ms"
  ]
}
```

Any tool or reviewer that classifies a golden diff as semantic or volatile reads
that file. Do not restate the list anywhere else.

## Panic reporting

The `rust-panic-safety` skill owns the redacted `report_panic` block and the
rule that the application-owned outermost bootstrap owns the process-global
hook. An embedded component exposes a redacted handler and never replaces the
hook. This section covers only the telemetry side of that record.

- Route the record through the same platform sink as other events. The event
  name and the site code come from closed vocabularies. The line and column are
  bounded integers.
- The hook runs on the panicking thread. Keep it allocation-light and lock-free
  where you can. A lock that the panicking thread already holds deadlocks here.
- The platform writer accepts structured fields, not one formatted string. Cap
  the encoded record before it reaches a platform-specific line limit.
- A debug or opt-in build does not make a raw panic payload safe. Keep the same
  redaction contract in every profile.
- On Android, a panic message also reaches the tombstone; the `rust-debugging`
  skill has the detail.

## Stall detection

A host-side "not responding" state has no direct Rust signal. The measurable
proxy is an async runtime that stops making progress.

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub static HEARTBEAT_TICK: AtomicU64 = AtomicU64::new(0);

pub fn spawn_heartbeat() {
    tokio::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            HEARTBEAT_TICK.fetch_add(1, Ordering::Relaxed);
        }
    });
}
```

The host reads `HEARTBEAT_TICK` through the same snapshot it already polls. It
records the last observed value and the host's own monotonic time when that
value changes. An unchanged value for a threshold — ten seconds is a workable
default — means the runtime is not scheduling. Raise the alert on the host
side. Do not compare wall-clock timestamps: clock corrections can create false
stalls or hide real ones.

The heartbeat detects a stalled scheduler. It does not detect one blocked worker
while another worker still runs the timer task. For that, add Tokio's runtime
metrics to the snapshot. Since tokio 1.45 these need no `tokio_unstable`:
`num_workers`, `num_alive_tasks`, `global_queue_depth`, and, on targets with
64-bit atomics, `worker_park_unpark_count` and `worker_total_busy_duration`.

```rust
use tokio::runtime::Handle;

pub struct WorkerSample {
    pub park_unpark_count: u64,
    pub busy_nanos: u128,
}

pub struct RuntimeSample {
    pub global_queue_depth: usize,
    pub workers: Vec<WorkerSample>,
}

pub fn sample_runtime(handle: &Handle) -> RuntimeSample {
    let metrics = handle.metrics();
    let workers = (0..metrics.num_workers())
        .map(|worker| WorkerSample {
            park_unpark_count: metrics.worker_park_unpark_count(worker),
            busy_nanos: metrics.worker_total_busy_duration(worker).as_nanos(),
        })
        .collect();
    RuntimeSample {
        global_queue_depth: metrics.global_queue_depth(),
        workers,
    }
}
```

A worker publishes these values when it parks or runs maintenance. Compare two
samples one threshold apart. An even `park_unpark_count` means the worker is
active. When it is even and unchanged, and the busy time did not advance, the
worker has been inside one poll the whole time: something blocks it. A growing
`global_queue_depth` confirms that work waits behind it.

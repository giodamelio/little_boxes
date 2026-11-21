# tokio-console setup

Deep material for `rust-debugging`, section 5. Read it before you add `tokio-console` to a local
debug build. Never ship it: it opens a local TCP port.

1. Add `console-subscriber` to the binary crate, enable `features = ["full", "tracing"]` on
   `tokio`, and call `console_subscriber::init()` first in `main`. `init()` installs the global
   subscriber with its own `fmt` layer, and it panics when a subscriber is already set. Remove
   the host `fmt()...init()` subscriber, or keep your subscriber and add
   `console_subscriber::spawn()` to a `tracing_subscriber::registry()` as a layer.
2. Add the cfg to the rustflags in `.cargo/config.toml`, not to `RUSTFLAGS`, which replaces
   every config-file rustflag. Add it to the rustflags table that the workspace already uses,
   because a matching `[target.<triple>]` table replaces `[build]` rustflags.

Remove all four changes, the rustflags entry included, before you commit. A committed
`tokio_unstable` flag reaches every CI and release build.

```toml
# .cargo/config.toml
[build]
rustflags = ["--cfg", "tokio_unstable"]
```

```bash
cargo install --locked tokio-console
cargo run --locked -p my-cli -- <args>
tokio-console        # in a second terminal
```

## Warning triage

| tokio-console shows | Cause | Next step |
|---|---|---|
| `This task has lost its waker, and will never be woken again.` | A `poll` returned `Pending` and dropped every waker clone | Before `poll` returns `Pending`, store `cx.waker().clone()` where the event source can call `wake`. Replace the stored waker when `will_wake` returns `false` |
| `This task has never yielded (...)`, threshold 1 s | Blocking work holds a worker thread | Move it to `spawn_blocking` (the `rust-async-internals` skill has the blocking-call table) |
| `This task has woken itself for more than 50% of its total wakeups` | A self-wake loop: the task busy-polls | Wait on a real event instead of waking itself |
| An idle task, never polled again, no warning | A channel or a lock that nobody releases | Backtrace every thread and find the holder |

## On a device or in production

On a device or a production build, where tokio-console cannot ship, read the stable
`tokio::runtime::RuntimeMetrics` instead: `num_alive_tasks`, `global_queue_depth`, and the
per-worker `worker_total_busy_duration` and `worker_park_unpark_count` (targets with 64-bit
atomics only). The `rust-observability` skill
covers how to export them.

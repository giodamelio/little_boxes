# Manual Poll Bridge

Reference material for `SKILL.md`. Read it when you drive tokio streams from a synchronous,
step-driven loop, or when you drive an io_uring backend from that loop.

Contents: triage; poll pattern; no-op waker; invariants; bridge extensions;
io_uring buffer ownership and completion.

## Triage

| Symptom | Likely cause | Check or fix |
|---|---|---|
| A stream stalls forever after one `Pending` | a no-op-waker poll helper was called from async code | move the call into the synchronous loop tick |
| Writes succeed, the peer sees nothing | the synchronous engine did not run a step after the write | check that the loop does not skip ticks under load |
| io_uring operations hang or corrupt memory after cancellation | an operation abandoned without draining its completions, or its buffer reused early | drain the target CQE and, for `SendZc` with `IORING_CQE_F_MORE`, its memory-release notification |

## The pattern

The bridge is a manual poll with the standard no-op waker, `Waker::noop()` (Rust 1.85). It
allocates nothing. Clippy's `manual_noop_waker` lint (as of clippy 1.99) flags only an `impl Wake`
with an empty `wake`. It does not flag a hand-written `RawWakerVTable` no-op, so review that form
by hand and replace it with `Waker::noop()`.

```rust
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use tokio::io::{AsyncRead, ReadBuf};

/// Call only from the synchronous loop tick, never from async code.
fn try_read_duplex(
    stream: &mut tokio::io::DuplexStream,
    buf: &mut [u8],
) -> Option<io::Result<usize>> {
    let mut cx = Context::from_waker(Waker::noop());
    let mut rb = ReadBuf::new(buf);
    match Pin::new(stream).poll_read(&mut cx, &mut rb) {
        // With a non-empty `buf`, a length of 0 is EOF.
        Poll::Ready(Ok(())) => Some(Ok(rb.filled().len())),
        Poll::Ready(Err(e)) => Some(Err(e)),
        Poll::Pending => None, // the loop retries on the next tick
    }
}
```

## Why the waker does nothing

The `try_*` functions run inside the synchronous loop tick, not inside a tokio task. No task
exists that a wake could reschedule, so a real waker would be wasted work.

The trade-off is that `Poll::Pending` carries no progress information. The loop decides when to
poll again, from its own timing, not from tokio wake signals.

## Invariants

1. **Do not call a no-op-waker poll helper from inside an async task, and do not wrap it in an
   `async fn`.** Under a no-op waker, `Poll::Pending` means that no wake will ever arrive. The
   task stalls for ever. Call these helpers only from synchronous loop code.
2. **`Pin::new` needs `Unpin`.** `Pin::new(&mut stream)` is sound here because
   `DuplexStream: Unpin`. A stream type that is not `Unpin` must be pinned once, for example in
   a `Pin<Box<_>>` field, and polled through `as_mut()`.
3. **Cancellation is cooperative, not waker-driven.** The loop checks a `CancellationToken`
   between polling rounds.

## Extend the bridge

- A new stream type implements `AsyncRead + AsyncWrite + Unpin`.
- `poll_read` returns `Poll<Result<()>>`, not a byte count. A read wrapper records
  `ReadBuf::filled().len()` before the poll. On `Poll::Ready(Ok(()))`, no increase means EOF.
  Translate EOF to `UnexpectedEof` only when the protocol requires more bytes.
- `poll_write` returns a byte count. A write wrapper translates `Poll::Ready(Ok(0))` into
  `WriteZero`, and handles `BrokenPipe` as a closed peer.

## io_uring registered buffers

When the loop drives an io_uring backend with registered buffer pools for zero-copy `SendZc` and
fixed-buffer `ReadFixed` (`IORING_OP_READ_FIXED`), hold these invariants:

1. Register buffers once at startup, and reference them by index in every SQE. A second
   `IORING_REGISTER_BUFFERS` fails with `EBUSY` while buffers are registered. Replace a slot with
   `IORING_REGISTER_BUFFERS_UPDATE` (Linux 5.13+); the kernel can keep the old buffer alive
   until the requests that use it complete. Before Linux 5.13, registration waits for the ring
   to go idle.
2. Keep each buffer owned and unavailable for reuse until every request releases it. `ReadFixed` releases
   its buffer at the target CQE. `SendZc` can produce two CQEs: the first reports the send
   result; `IORING_CQE_F_MORE` on that CQE means the buffer is still in use. Wait for the
   notification CQE with `IORING_CQE_F_NOTIF` before reuse or deallocation. Without `F_MORE`,
   the send-result CQE releases the buffer. Track these states per request, not per CQE count.
3. tokio does not synchronize SQE submission and CQE completion. The loop polls the ring and
   drains completions into per-consumer queues.
4. Every `unsafe` block that constructs an SQE carries a `// SAFETY:` comment that names the
   buffer-index validity and the fd lifetime. The `rust-unsafe` skill, when it is installed, owns
   the convention.
5. To stop an in-flight operation, submit `IORING_OP_ASYNC_CANCEL` and drain its completions.
   The cancel CQE is not the target CQE and does not release the target's buffer. Keep the fd
   owned until the target completes, and keep the buffer until the release condition in rule 2.
   This includes a pending `SendZc` notification after the send result. `-EALREADY` means the
   request is still running; `-ENOENT` can mean its result is already queued. Neither cancel
   result permits early reuse.

Verify on the supported Linux kernel with the real backend: exercise normal completion,
cancel before completion, and a send-result CQE with `F_MORE` followed by `F_NOTIF`. Assert
that the pool does not return a buffer before release. A compile check or a successful cancel
CQE does not prove this lifetime contract.

Sources: [zero-copy send completions](https://github.com/axboe/liburing/blob/master/man/io_uring_prep_send_zc.3),
[cancellation results](https://github.com/axboe/liburing/blob/master/man/io_uring_prep_cancel.3).

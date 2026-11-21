# Deadline phases and retry delay

Contents: phase caps; idle and total timeouts; retry classification; replayable uploads; retry budget; full-jitter
delay; sources.

## Phase caps

Each cap bounds one failure mode. The operation deadline bounds all of them together. Give each
phase `min(configured phase cap, operation deadline - monotonic now)`.

| Control | Starts | Ends | Protects against |
|---|---|---|---|
| DNS cap | Before lookup | Address set returned | Resolver stall |
| Connect cap | Before socket or proxy connect | Secure connection ready | Route, proxy, TCP, or TLS stall |
| Write idle cap | After write starts | Request body complete | Peer that stops reading |
| First-byte cap | After request complete | Response headers arrive | Slow handler or upstream |
| Read idle cap | After body starts | Each body chunk | Peer that stops sending |
| Operation deadline | At API entry | Body consumed or discarded | All cumulative work |

## Idle and total timeouts

An idle timeout is not a total transfer timeout. Reset it only after useful
progress. A peer that sends one byte before every idle timeout can still consume
the operation deadline, so enforce both. With tower-http, pair
`RequestBodyTimeoutLayer` (idle) with `RequestBodyDeadlineLayer` (total,
tower-http 0.7+). With reqwest, pass the remaining time to
`RequestBuilder::timeout` on every attempt.

## Retry classification

HTTP `GET`, `HEAD`, `PUT`, `DELETE`, `OPTIONS`, and `TRACE` have idempotent
method semantics. Application behavior can still make an operation unsafe to
repeat. Do not infer replay safety from the method name alone. Do not retry
`POST` unless the application contract provides idempotency or proves that the
server did not apply the request.

Use this default classification, then narrow it for the target service:

| Result | Automatic retry |
|---|---|
| DNS temporary failure | Yes, if another attempt fits |
| Connect refused, reset, or timeout | Yes, for an eligible operation |
| Connection closes before response headers | Yes, for an eligible operation |
| HTTP `408`, `429`, `502`, `503`, or `504` | Yes, when service policy permits it |
| HTTP `Retry-After` | Wait at least the valid server delay, within the deadline |
| Other `4xx` | No |
| TLS certificate or hostname failure | No |
| Request encode or validation failure | No |
| Body stream failed after response headers | No by default |
| Cancellation or exhausted deadline | No |

## Replayable uploads

For uploads, make replayability explicit. A byte buffer is replayable. A file is
replayable only when each attempt opens a new handle and starts at the same
offset. A live channel, socket, decoder, or one-shot generator is not
replayable.

## Retry budget

Bound retries per upstream across the process. Require a retry permit or token before a second
attempt. Limit retry concurrency, and suppress retries when the local service is overloaded or the
upstream failure rate crosses the configured threshold. If no retry budget remains, return the
original failure instead of adding outage load. `tower::retry::budget::TpsBudget` implements such a
budget for a tower stack.

## Full-jitter delay

Keep the delay calculation pure. Pass the random sample in, so a test can fix it. For retry number
`n`, starting at zero, the function selects a delay in `0 ..= min(maximum, base * 2^n)`. The shift
and the multiplication saturate, so a large retry number gives at most `maximum` and does not
overflow.

```rust,run
use std::time::Duration;

fn full_jitter_delay(
    base: Duration,
    maximum: Duration,
    retry_number: u32,
    sample: u64,
) -> Duration {
    let factor = 1_u128.checked_shl(retry_number).unwrap_or(u128::MAX);
    let cap = base.as_millis().saturating_mul(factor);
    let cap = cap.min(maximum.as_millis()).min(u128::from(u64::MAX)) as u64;
    Duration::from_millis(if cap == u64::MAX {
        sample
    } else {
        sample % (cap + 1)
    })
}

fn main() {
    let (base, maximum) = (Duration::from_millis(100), Duration::from_secs(2));
    assert!(full_jitter_delay(base, maximum, 3, 900) <= Duration::from_millis(800));
    // A large retry number saturates at `maximum` instead of overflowing.
    assert!(full_jitter_delay(base, maximum, 200, u64::MAX) <= maximum);
}
```

Apply `Retry-After` to the result. Take the larger of the valid server delay and this delay. If
that delay and one more attempt do not fit in the time left before the operation deadline, do not
retry. Return the original failure.

## Sources

- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html): idempotent methods, `408`, `Retry-After`.
- [RFC 6585](https://www.rfc-editor.org/rfc/rfc6585.html): `429 Too Many Requests`.
- [tower retry budget](https://docs.rs/tower/latest/tower/retry/budget/index.html): process-wide
  retry limits.

# Network test seams

Test policy without a real public network. Use the smallest seam that exposes the behavior under
test.

| Behavior under test | Seam |
|---|---|
| Deadlines and backoff | Pause or inject monotonic time |
| Jitter | Pass a fixed jitter sample into the delay calculation |
| Partial writes, delayed headers, stalled bodies | Run a local listener |
| Multiple addresses, DNS failures | Inject a resolver result |
| TLS name and trust failures | Generate a test CA and a server certificate |
| Body replay | Use a counting body that fails if a retry reads it twice |
| Pool reuse, drain after a policy change | Expose pool-connect counts |
| Overload rejection | Hold a concurrency permit |
| Shutdown deadline | Keep one handler open; assert that the serve future returns within the shutdown deadline |
| Header-read timeout | Open one connection that sends nothing and one that sends a partial request header; assert that the server closes both after the header timeout |

## Test deterministic seams

`#[tokio::test(start_paused = true)]` needs tokio's `test-util` feature. Do not
combine `start_paused` with real sockets: auto-advance fires the deadline while
the socket waits.

Do not assert only the final error string. Assert the attempt count, elapsed
virtual time, bytes consumed, connection count, and error class.

Run the smallest affected tests first. Then run the workspace network tests
once, with test-runner retries off (for cargo-nextest, pass `--retries 0`). A
runner retry can hide a flaky network contract.

## Sources

- [tokio time](https://docs.rs/tokio/latest/tokio/time/): timeout cancellation and paused clocks.

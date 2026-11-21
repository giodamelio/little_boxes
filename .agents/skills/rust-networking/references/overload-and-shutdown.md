# Server overload and shutdown

Admission limits and the shutdown sequence that the "Conditional references" table in
`SKILL.md` points to.

## Bound server overload

Put a finite limit on accepted connections, incomplete request headers,
in-flight requests, request-body bytes, response-body bytes when applicable,
and queued work. Apply header read and keep-alive limits before application
admission. Apply a tighter concurrency limit to expensive routes.

Acquire the concurrency permit before expensive parsing, decompression,
database work, or downstream calls. Keep the wait queue bounded. When capacity
is exhausted, reject promptly with the protocol's overload response. For HTTP,
use `503 Service Unavailable` and a valid `Retry-After` when the server can give
a useful delay.

Answer a handler timeout with `503` or `504`, never `408`. `408` tells the
client that the server did not receive the whole request, so a client can
repeat even a `POST` after the handler already ran. With tower-http, use
`TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, timeout)`.

Do not combine a large buffer with a concurrency limit and call the result
bounded. The buffer is admitted work too. Include queued requests in the memory
and deadline budget.

## Shut down in a fixed order

Use this server shutdown sequence:

1. Mark the instance unready in service discovery or the load balancer.
2. Stop accepting new connections.
3. Stop admitting new requests on persistent connections.
4. Signal accepted handlers to finish.
5. Drain handlers and response bodies until the shutdown deadline.
6. Cancel the remaining work after the deadline.
7. Await task termination and release listeners, pools, and permits.

Keep one finite shutdown deadline. Start it when the shutdown signal arrives,
not when the server starts. Do not apply a new full grace period at each step.
A signal storm must not restart the deadline. axum `with_graceful_shutdown`
covers steps 2 to 5 but has no deadline; bound it from the signal as
[stack-defaults.md](stack-defaults.md) shows. Track
spawned background work with `tokio_util::task::TaskTracker` (`close()`, then
`wait()`) so the drain covers it.

For clients, stop accepting new operations, let eligible in-flight operations
finish within the deadline, then close pools. Do not drop a runtime while
network tasks still own sockets.

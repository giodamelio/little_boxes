---
description: Use when building or reviewing a production Rust network client or server with reqwest, hyper, axum, tower-http, tonic, or rustls; setting an HTTP timeout or deadline budget; retrying with Retry-After and jitter; keeping TLS verification on; configuring a connection pool, proxy, or DNS; enforcing a response body limit or overload bound; or adding graceful shutdown. Not for select! or task cancel safety; use `rust-async-internals`.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-networking
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 7c3fc56f87714af27579cf99bdce25522e2020e4
name: rust-networking
---
# Rust Networking

Apply this policy to HTTP, gRPC, WebSocket, and custom TCP clients and servers.
Use the controls of the stack that the workspace already uses; do not replace
the framework for one middleware type. The `rust-async-internals` skill owns
`select!` and cancel safety; without it, assume a dropped future stops at any
`.await`. The `rust-security` skill owns parser hardening and the advisory gate,
`cargo deny --config deny.toml --locked check advisories`.

## Start from one operation contract

Record these decisions next to the configuration before you tune settings.

| Field | Required decision |
|---|---|
| Operation deadline | Wall-clock time for all attempts and delays |
| Phase caps | DNS, connect, write idle, first-byte, and read idle caps |
| Retry attempts | Total attempts, the first included |
| Retry eligibility | Operation semantics, replayable body, and retryable result |
| Response limit | Decoded bytes the caller accepts |
| Concurrency limit | Admitted work per process, route, or upstream |
| Server header limit | Header bytes, field count, and read time |
| Server idle limits | Keep-alive, request-body, response-write idle caps |
| Shutdown grace | Maximum time for accepted work to drain |

Derive timeouts from the caller's deadline and the latency objective, not from
another service. Keep each one configurable, with a finite production default.

## Set the limits the stack leaves open

An unset limit passes tests, then fails under a slow or hostile peer. Set each
one explicitly.

| Stack | Default | Consequence |
|---|---|---|
| reqwest 0.13 async `Client` | No `timeout`, `connect_timeout`, or `read_timeout`; `pool_max_idle_per_host` is `usize::MAX` | A stalled peer holds a request forever; the idle pool is unbounded |
| reqwest `Response::bytes`, `text`, `json` | Collect the whole body with no size limit | A large response exhausts memory |
| hyper 1.x HTTP/1 server | `header_read_timeout` (30 s) acts only after `Builder::timer`; a configured value with no timer panics | Slow-header clients hold connections |
| `axum::serve` 0.8 | Builds its hyper builder with no timer | No header-read timeout at all |
| axum `with_graceful_shutdown` | Waits for open connections with no deadline | One open request stalls shutdown |
| tower-http `TimeoutLayer::new` | Deprecated since 0.6.7; answers `408` | A client can repeat a `POST` that already ran |
| tonic 0.14 `Endpoint` | No connect timeout; `timeout` does not send `grpc-timeout` | The server never sees the deadline |
| rustls 0.23 | Zero or two provider features (or `custom-provider`) and no installed provider | `ClientConfig::builder()` panics at run time |

Read [references/stack-defaults.md](references/stack-defaults.md) when the
workspace uses one of these crates: it has the reqwest 0.13 upgrade changes and
working client, server, rustls, and Android TLS setups.

## Verify the configuration

Run these checks when you add or review network configuration. A hit is not
always a defect; review each one.

```bash
rg -n --type rust 'danger_accept_invalid|tls_danger_|\.dangerous\(\)|tls_sslkeylogfile\(true\)|KeyLogFile|DefaultBodyLimit::disable|\bTimeoutLayer::new|axum::serve\(|with_graceful_shutdown\(|\breqwest::get\(|\bClient(Builder)?::(new|builder)\(\)|\.(bytes|text|json)\(\)\.await|\.json::<'
if cargo tree --locked -i rustls --depth 0 > /dev/null 2> rustls.err; then
  cargo tree --locked -e features -i rustls --prefix none | grep -oE '^rustls feature "(ring|aws_lc_rs|custom-provider)"' | sort -u
elif ! grep -q 'did not match any packages' rustls.err; then cat rustls.err; exit 1; fi
cargo deny --config deny.toml --locked check advisories
```

| Check | Proves | Does not prove |
|---|---|---|
| `rg` | Each bypass, key log, unbounded body, `408` timeout, and unbounded construction is visible | That the values fit the contract |
| `cargo tree` | Which rustls provider features are on. Output other than one `ring` or `aws_lc_rs` line means `main` must install a provider | That the provider is installed before first TLS use |
| `cargo deny` | No known vulnerability in the graph; a transitive unsound advisory fails only with `unsound = "all"`. Without cargo-deny, run `cargo audit --deny warnings` | Protocol correctness |

A green grep does not prove the timing contract; the tests in
`references/test-seams.md` do. The work is complete when each contract field and
each stack-defaults row has a finite, configurable, tested value. In a review,
each gap is a finding.

## Failure triage

| Symptom | Likely cause | First check |
|---|---|---|
| Request exceeds its advertised timeout | Each retry received a fresh timeout | Trace one absolute deadline across attempts |
| POST executes twice | Method-only retry policy | Check idempotency and body replayability |
| Retry storm during outage | No jitter, no attempt cap, or ignored `Retry-After` | Inspect delay and total attempt metrics |
| TLS works only in development | Another production root source, or a test verifier bypass | Print root source and certificate error class |
| Requests use the old endpoint | Live pooled connection survives DNS change | Compare pool reuse and connect events |
| Memory grows under slow clients | Body collection or unbounded queue | Inspect decoded-byte and queue limits |
| HTTP/2 overloads the service | Connection limit is used as request limit | Inspect in-flight streams and route permits |
| Shutdown never finishes | New work still admitted, grace reset per step, or no outer deadline | Trace admission close and one deadline |
| Slow clients exhaust server connections | No header-read timeout (`axum::serve`, hyper with no timer) | Send zero bytes and a partial header; time each close |
| Panic "Could not automatically determine the process-level CryptoProvider" | The rustls row of the limits table | Run the `cargo tree` check; install one provider in `main` |
| Metrics leak user data | Raw URI, header, or error text as a label | Use a closed field vocabulary |

## Keep TLS verification on

- Verify the certificate chain and the original server name (not the resolved
  IP address) on every production connection, and send SNI for a DNS name.
- Load trust roots from one explicit source: the platform store for managed
  enterprise roots, a bundled store for the same roots on every target. A
  reqwest 0.12 `rustls-tls` build silently moves to the platform verifier on
  0.13; call `tls_certs_only(roots)` to keep a bundled or private set. Fail
  client construction when the required root store is empty.
- Keep the bypass APIs that the `rg` check lists out of production code. A test
  verifier must not be reachable from runtime configuration.
- Never enable key logging in production: reqwest `tls_sslkeylogfile(true)`
  and rustls `KeyLogFile` write session secrets to the `SSLKEYLOGFILE` file.
  Keep private keys out of logs and error chains.
- After a trust-root, client-certificate, or key rotation, rebuild the client
  and drain old pooled connections.
- Pin certificates only with a rotation plan and a shipped backup identity.
- Keep the backend's protocol-version defaults unless every peer is proven to
  support a higher minimum.

## Use one absolute deadline

Create one monotonic deadline at the operation boundary. Carry it through DNS,
connect, proxy, TLS, every attempt, response streaming, and retry delay. Do not
restart the full timeout after a retry. Before each phase, compute
`min(configured phase cap, operation deadline - monotonic now)`, fail before
the phase starts when no time remains, and report which phase consumed the
budget, not `connect timeout` for all of them.

On a server, start the header-read deadline at accept, before protocol
detection: a client that sends nothing must not hold a connection slot.

An idle timeout is not a total transfer timeout: a peer that sends one byte
before every idle timeout can still consume the operation deadline, so enforce
both. Use `tokio::time::timeout_at` when the stack does not accept a deadline;
dropping the timed future cancels it, so check its cancel safety. Read
[references/deadlines-and-retries.md](references/deadlines-and-retries.md) when you set the phase caps or the
idle and total pair.

## Retry only a safe operation

Retry only when all three conditions hold:

1. The operation is idempotent by protocol semantics or by an idempotency key
   that the server stores atomically.
2. The complete request can be replayed byte for byte. A consumed stream cannot.
3. The failure is transient, and the deadline can hold another attempt plus its
   delay.

Do not infer replay safety from the method name. Do not retry `POST` unless the
contract provides idempotency or proves the server did not apply the request. Do not retry every `5xx`: keep the retry status set explicit.
Count the first request as attempt one, keep the attempt limit small, and
never retry forever in a background task. Bound retries per upstream with a
retry budget (`tower::retry::budget::TpsBudget`).

Use exponential backoff with full jitter. Wait at least a valid `Retry-After`
delay, never past the deadline. Read
[references/deadlines-and-retries.md](references/deadlines-and-retries.md) for the result classification, the
`Retry-After` rules, and a pure, tested delay function.

## Stream with hard limits and backpressure

Reject a declared body length over the route limit before allocation, then
count the actual decoded bytes as chunks arrive and stop at the limit: a small
compressed body can expand into a large decoded one. Process large bodies as
streams. On a server, use `http_body_util::Limited`,
`axum::body::to_bytes(body, limit)`, or a per-route `DefaultBodyLimit::max(n)`.
Limit and record both wire and decoded sizes when the stack exposes them. Put a
bounded channel between network reads and a slower consumer. Never add an
unbounded queue to hide backpressure.

Read [references/deadlines-and-retries.md](references/deadlines-and-retries.md)
when an upload must be replayable, and [references/cancellation.md](references/cancellation.md)
before you pool a connection after a cancelled or failed read.

## Emit safe network diagnostics

Record bounded fields: route template (not the full URL), method, attempt,
retry reason class, timeout phase, status class, byte counts, TLS version, and
certificate error class. Do not record URL
queries, raw paths with identifiers, headers, cookies, tokens, certificate
contents, body fragments, proxy credentials, or peer error text that can echo
them. Do not put the remote address into a metric label. The
`rust-observability` skill, when it is installed, owns the redaction gates.

## Conditional references

Test policy without a real public network.

| Read | When |
|---|---|
| `references/cancellation.md` | A caller can cancel a state-changing request, or must recover from an `Outcome unknown` result |
| `references/proxy-and-dns.md` | You key or reuse a connection pool, or configure a proxy, `NO_PROXY`, a resolver, or a DNS cache |
| `references/overload-and-shutdown.md` | You set server admission, concurrency, queue, or handler-timeout limits, or add or review graceful shutdown |
| `references/test-seams.md` | You write a network policy test |

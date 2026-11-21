# Stack defaults and setup

API claims were checked in 2026-09 against reqwest 0.13.5, tonic 0.14.6, rustls 0.23.45, and
rustls-platform-verifier 0.7.0, and re-checked in 2026-11 against hyper 1.12.0, hyper-util 0.1.21,
axum 0.8.9, and tokio 1.54.1. The two claims marked "probe" were re-run on rustc 1.99.1 with that
2026-11 set. The tower-http rows were read in the tower-http 0.7.1 source; the compile harness
holds tower-http 0.6.11, so no example here compiles against the 0.7 deadline layers. The
reqwest, axum, and hyper-util blocks are type-checked against the versions above. The rustls
block was type-checked separately against rustls 0.23.45 on rustc 1.99.1, because its
default aws-lc-rs provider does not build in every cross-target check.

Contents:

- [reqwest client](#reqwest-client)
- [rustls crypto provider](#rustls-crypto-provider)
- [Android platform verifier](#android-platform-verifier)
- [axum and hyper servers](#axum-and-hyper-servers)
- [tower-http timeouts](#tower-http-timeouts)
- [tonic](#tonic)

## reqwest client

These defaults apply to the async `reqwest::Client`. `reqwest::blocking::Client` has a 30 s total
timeout by default.

| Setting | 0.13 default | Set it to |
|---|---|---|
| `timeout` (connect to end of body) | none | the operation deadline |
| `connect_timeout` | none | the connect cap |
| `read_timeout` (resets after each read) | none | the read idle cap |
| `pool_max_idle_per_host` | `usize::MAX` | a finite per-host idle limit |
| `pool_idle_timeout` | 90 s | the idle lifetime of the contract |

Pass the remaining time of the operation deadline to `RequestBuilder::timeout` on every attempt.
The per-request value replaces the client value for that request. The values below are
placeholders; derive them from the operation contract.

```rust
use std::time::{Duration, Instant};

fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .read_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(16)
        .pool_idle_timeout(Duration::from_secs(60))
        .build()
}

#[derive(Debug)]
enum FetchError {
    DeadlinePassed,
    Http(reqwest::Error),
}

async fn fetch(
    client: &reqwest::Client,
    url: &str,
    deadline: Instant,
) -> Result<reqwest::Response, FetchError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(FetchError::DeadlinePassed);
    }
    client.get(url).timeout(remaining).send().await.map_err(FetchError::Http)
}
```

Return a deadline error when no time is left, instead of starting an attempt with a zero
timeout that reports a transport timeout for a deadline the caller already spent.

`Response::bytes()`, `text()`, and `json()` collect the whole body with no size limit. For an
untrusted or unbounded peer, read with `Response::chunk()`, count the bytes, and stop at the limit.

`ClientBuilder::retry` (0.12.23+) retries protocol NACKs by default, up to 2 times after the first
attempt. A `reqwest::retry::for_host(..)` policy replaces that default and adds a 20% retry budget
(tower `TpsBudget`). It retries at once, with no backoff and no `Retry-After` handling. Use it only
for immediate safe retries. Do not also wrap it in an outer retry loop, because the attempts
multiply.

An upgrade from 0.12 to 0.13 changes these defaults (reqwest CHANGELOG, v0.13.0):

| Change | Effect on the code |
|---|---|
| rustls is the default backend, not native-tls | The TLS stack and its error text change |
| The rustls crypto provider is aws-lc-rs, not ring | The heavier aws-lc-sys C build replaces the ring build; a cross build needs a target C toolchain. See the provider section |
| rustls-platform-verifier supplies the roots by default | A 0.12 `rustls-tls` build (bundled webpki roots) moves to the platform store. Call `tls_certs_only(roots)` for a bundled or private root set |
| Feature `rustls-tls` is renamed `rustls` | Old feature lists fail to resolve |
| `query` and `form` are opt-in features | `RequestBuilder::query` and `form` stop compiling until you enable them |
| TLS builder methods get `tls_` names | `danger_accept_invalid_certs` becomes `tls_danger_accept_invalid_certs`; the old names stay as soft deprecations |

HTTP/3 (`http3` feature) is unstable. It compiles only with `--cfg reqwest_unstable` in the rustflags.
Put the flag in `.cargo/config.toml`: a `RUSTFLAGS` variable replaces every config-file rustflags
value. Do not make HTTP/3 the default production path.

## rustls crypto provider

rustls 0.23 picks the process-level `CryptoProvider` from its crate features only when exactly one
of `ring` and `aws_lc_rs` is on and `custom-provider` is off. In every other case, when no provider
is installed, `ClientConfig::builder()`,
`ServerConfig::builder()`, and `WebPkiServerVerifier::builder()` panic at run time with "Could not
automatically determine the process-level CryptoProvider from Rustls crate features". reqwest 0.13
builds its own aws-lc-rs provider when none is installed, so the panic appears in another crate
that uses rustls, at its first TLS use.

Check the enabled features:

```bash
if cargo tree --locked -i rustls --depth 0 > /dev/null 2> rustls.err; then
  cargo tree --locked -e features -i rustls --prefix none | grep -oE '^rustls feature "(ring|aws_lc_rs|custom-provider)"' | sort -u
elif ! grep -q 'did not match any packages' rustls.err; then cat rustls.err; exit 1; fi
```

The first command checks that rustls is in the graph. Without it, `cargo tree -i rustls` exits
101 with "did not match any packages", the pipe hides the exit status, and the empty output reads
like the zero-provider case. Any other failure, such as an ambiguous name when two rustls
versions resolve, stops the check. `-o` drops the ` (*)` suffix that `cargo tree` adds to repeated lines, so each feature prints once.
If zero lines, two provider lines, or `custom-provider` appear, install one provider at the top of
`main`, before any TLS client or server exists. With more than one rustls version in the graph,
name it: `-i rustls@0.23.45`.

```rust,ignore
fn main() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("no other rustls CryptoProvider is installed yet");
}
```

## Android platform verifier

rustls-platform-verifier is the reqwest 0.13 default and calls the Android certificate verifier
through JNI. It needs three things, or certificate verification panics with "Expect
rustls-platform-verifier to be initialized":

1. Call `rustls_platform_verifier::android::init_with_env(&mut env, context)` (or
   `init_with_runtime`) once, before the first TLS connection. Both names exist in 0.6 and 0.7.
   The README still shows `init_hosted` and `init_external`: 0.6 deprecated them and 0.7 removed
   them. 0.7 takes jni 0.22 types. Depend on the same rustls-platform-verifier version that reqwest
   resolves (`cargo tree --locked -i rustls-platform-verifier`); a second copy has its own
   uninitialized global.
2. Add the crate's Kotlin component to the Gradle build (its README has the Maven setup).
3. Keep the classes when the app uses R8 or ProGuard:

```text
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
```

The `rust-jni` skill, when it is installed, owns the JNI entry point: guard each export with
`native_method!` or `with_env`, because a panic that reaches an unguarded export aborts. The
`rust-android-build` skill owns the packaging: build each ABI through a Gradle task that writes
`<generated-dir>/jniLibs/<abi>/libnative.so`.

## axum and hyper servers

| API | Default | Effect |
|---|---|---|
| hyper `http1::Builder::header_read_timeout` | 30 s | Applies only after `Builder::timer(...)`. With no timer, hyper applies no limit and logs nothing (the warning needs hyper's unstable `tracing` feature). A configured value with no timer panics |
| `axum::serve` | Builds the hyper builder with no timer | No header-read timeout (probe: a partial header stayed open for the whole 4 s window; hyper-util with a 1 s timer closed it at 1 s) |
| `with_graceful_shutdown` | Waits for every open connection | No deadline (probe: one slow request kept shutdown running for the whole 4 s window) |
| `Bytes`, `String`, `Json`, `Form` extractors | 2 MB body limit | `DefaultBodyLimit::max(n)` changes it; `DefaultBodyLimit::disable()` removes it |

For a manual body read, use `axum::body::to_bytes(body, limit)` or wrap the body in
`http_body_util::Limited`.

The unreleased next axum major (CHANGELOG "Unreleased", 2026-09) applies hyper's default
header-read timeout in `axum::serve` and adds `ListenerExt::limit_connections`. Check the
changelog again when you upgrade past 0.8.

Bound `with_graceful_shutdown` from the signal, not from server start. A `tokio::time::timeout`
around the whole serve future stops a healthy server after the grace period.

```rust
use std::time::Duration;

use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

async fn serve_with_deadline(
    listener: TcpListener,
    app: Router,
    shutdown: CancellationToken,
    grace: Duration,
) -> std::io::Result<()> {
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown.clone().cancelled_owned());
    tokio::select! {
        result = server => result,
        () = async {
            shutdown.cancelled().await;
            tokio::time::sleep(grace).await;
        } => Ok(()),
    }
}
```

After the deadline, the connection tasks that axum spawned still exist. They stop when the runtime
shuts down, so return from `main` promptly.

When slow-header protection must live in the process, not in a front proxy, run the accept loop
with hyper-util. The hyper header timer is not enough on its own: the `auto` builder first reads
bytes to choose HTTP/1 or HTTP/2, and that phase has no timer. A client that sends nothing, or
sends `PRI` and stops, holds the connection forever (hyper-util 0.1.21; `http1_only()` does not
change it). This loop bounds the whole phase from accept to the first request with one
deadline, keeps the hyper timer for the headers of later requests, and drains to one deadline.

```rust
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Router;
use hyper::service::{Service as _, service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder;
use hyper_util::server::graceful::GracefulShutdown;
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;

async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()>,
    grace: Duration,
) {
    let header_timeout = Duration::from_secs(10);
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(header_timeout);
    builder.http2().timer(TokioTimer::new());

    let graceful = GracefulShutdown::new();
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _peer)) = accepted else {
                    // EMFILE and similar errors repeat at once; do not spin.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                // Set when the first request reaches the service, so the
                // deadline below covers protocol detection and the first header.
                let started = Arc::new(AtomicBool::new(false));
                let seen = Arc::clone(&started);
                let tower = TowerToHyperService::new(app.clone());
                let service = service_fn(move |request| {
                    seen.store(true, Ordering::Relaxed);
                    tower.call(request)
                });
                let conn = builder
                    .serve_connection_with_upgrades(TokioIo::new(stream), service)
                    .into_owned();
                let conn = graceful.watch(conn);
                tokio::spawn(async move {
                    let mut conn = std::pin::pin!(conn);
                    tokio::select! {
                        _ = conn.as_mut() => return,
                        () = tokio::time::sleep(header_timeout) => {}
                    }
                    // No request yet: dropping the connection closes the socket.
                    if started.load(Ordering::Relaxed) {
                        let _ = conn.await;
                    }
                });
            }
            () = &mut shutdown => break,
        }
    }
    drop(listener);
    let _ = tokio::time::timeout(grace, graceful.shutdown()).await;
}
```

Measured with a 1 s deadline (hyper-util 0.1.21, Rust 1.99.1): a client that sends zero bytes,
`PRI`, or `GET / HT` is closed after 1.00 s, and a request whose handler takes 1.5 s still gets
its response. The loop without the flag left the first two open. This loop has no connection limit. Add a semaphore permit per accepted connection when the contract
sets one.

## tower-http timeouts

| Layer | Behavior |
|---|---|
| `TimeoutLayer::with_status_code(status, timeout)` | Ends a slow handler with `status`. Use `503` or `504` |
| `TimeoutLayer::new` | Deprecated since 0.6.7. Answers `408 Request Timeout`, which tells the client that the server did not receive the whole request, so a client can repeat even a `POST` after the handler already ran |
| `RequestBodyTimeoutLayer` | Idle cap; resets on every body frame |
| `RequestBodyDeadlineLayer`, `ResponseBodyDeadlineLayer` | Total cap on one body transfer (0.7.0+). Stops a peer that sends one byte per idle period |

Use an idle layer and a deadline layer together.

## tonic

| API | Default | Effect |
|---|---|---|
| `Endpoint::connect_timeout` | none | Set the connect cap |
| `Endpoint::timeout` | none | Applies locally only. It does not send `grpc-timeout`, so the server never sees the deadline: it cannot stop at it or pass it downstream. Call `Request::set_timeout` to send `grpc-timeout` |

tonic 0.14 moved prost support into the `tonic-prost` and `tonic-prost-build` crates.

## Sources

- [reqwest CHANGELOG](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md) and
  [`ClientBuilder`](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)
- [hyper `header_read_timeout`](https://docs.rs/hyper/latest/hyper/server/conn/http1/struct.Builder.html#method.header_read_timeout)
- [axum 0.8.9 `serve` source](https://github.com/tokio-rs/axum/blob/axum-v0.8.9/axum/src/serve/mod.rs)
  and [axum CHANGELOG](https://github.com/tokio-rs/axum/blob/main/axum/CHANGELOG.md)
- [tower-http CHANGELOG](https://github.com/tower-rs/tower-http/blob/main/tower-http/CHANGELOG.md)
- [tonic `Endpoint`](https://docs.rs/tonic/latest/tonic/transport/channel/struct.Endpoint.html)
- [rustls `CryptoProvider`](https://docs.rs/rustls/latest/rustls/crypto/struct.CryptoProvider.html)
- [rustls-platform-verifier](https://docs.rs/rustls-platform-verifier/latest/rustls_platform_verifier/)

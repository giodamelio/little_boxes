# Async Tests Under Paused Time

Extended reference for the `rust-tdd` skill. Read this when a test waits on time, or when a
test must assert a state while an operation is in flight.

## Runtime mistakes

Use `#[tokio::test]` for code that runs on tokio. It builds a fresh current-thread runtime
for each test. Three runtime mistakes fail or hang the test for a reason that is not the
behavior:

- A tokio timer or I/O type used outside a runtime, for example in a plain `#[test]` through
  `futures::executor::block_on`, panics with `there is no reactor running`.
- A `Runtime::block_on` inside `#[tokio::test]` panics with `Cannot start a runtime from
  within a runtime`. Use `.await` instead.
- Inside `#[tokio::test]`, a blocking wait such as `futures::executor::block_on` or
  `std::sync::mpsc::Receiver::recv` blocks the only runtime thread. Spawned tasks and timers
  then cannot run, so a wait on one of them hangs the test. Use `.await` instead.

## Paused time

For code that waits on time, start the test with paused time:

```rust
#[tokio::test(start_paused = true)]
async fn retry_backs_off_before_the_second_attempt() { /* ... */ }
```

The runtime then advances the clock to the next timer whenever no task is ready to run, so a
one-hour backoff finishes at once. The clock also jumps while a task waits on a real thread
or real I/O, so a `timeout` around that wait fires at once. Only a running `spawn_blocking`
task stops the jump. Keep the fakes that run under paused time on tokio primitives.

`start_paused` needs the tokio `test-util` feature and the current-thread flavor. The macro
rejects `flavor = "multi_thread"` with `start_paused` at compile time. Enable the feature in
`dev-dependencies` only, so it never reaches a release build:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt", "test-util"] }
```

## Assert an intermediate state

Do not sleep until the operation is "probably" in flight. Gate the fake with two `oneshot`
channels: one that signals that the fake was reached, one that releases it.

```rust
#[tokio::test]
async fn state_is_starting_while_transport_start_is_in_flight() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let transport = Arc::new(FakeTransport::gated(started_tx, release_rx));
    let service = Arc::new(Service::new(Arc::clone(&transport)));

    let task = tokio::spawn({
        let service = Arc::clone(&service);
        async move { service.start().await }
    });

    // The fake reached start() and now blocks.
    started_rx.await.expect("transport start was never called");
    assert_eq!(service.state(), State::Starting);

    // Release the fake and let the operation finish.
    release_tx.send(()).expect("task dropped the release channel");
    task.await.expect("task panicked").expect("start must succeed");
    assert_eq!(service.state(), State::Running);
}
```

Await every `JoinHandle` the test spawns, as the last lines do. Tokio catches a panic inside
a spawned task and reports it only through the `JoinHandle`. A test that never awaits the
handle passes, and the panic appears only as text in the captured output.

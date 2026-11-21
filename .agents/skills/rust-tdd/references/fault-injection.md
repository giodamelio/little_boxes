# Fault Injection Queue

Extended reference for the `rust-tdd` skill. Read this when you write a fake, or when you
test error paths, retry logic, or cancellation.

Contents: why a queue, reference implementation, target enum, a fake that consults the queue,
outcome selection, scope rules, asserting that the fault fired, a service-level fault test.

## Why a queue

Error paths need tests as much as happy paths. Three common approaches fail:

- **`if cfg!(test)` branches in production code.** The production code now has a shape that
  only tests use. The branch is never exercised in release builds.
- **A `should_fail: bool` field on each fake.** It cannot express "fail once, then
  succeed", which is what a retry test needs.
- **A panicking fake.** A panic tests the panic path, not the `Result` path.

One ordered queue per fake solves all three. The fake asks the queue for an outcome, and
the test decides which call fails, how, and how many times.

## Reference implementation

```rust
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultOutcome {
    Error,
    Timeout,
    Drop,
    Reset,
    MalformedPayload,
    BlankPayload,
    Panic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultScope {
    /// Fires once, then is consumed.
    OneShot,
    /// Fires on every matching call until `clear` is called.
    Persistent,
}

#[derive(Clone, Debug)]
pub struct FaultSpec<T> {
    pub target: T,
    pub outcome: FaultOutcome,
    pub scope: FaultScope,
    pub message: Option<String>,
    pub payload: Option<Vec<u8>>,
}

pub struct FaultQueue<T> {
    specs: Mutex<VecDeque<FaultSpec<T>>>,
}

impl<T> Default for FaultQueue<T> {
    fn default() -> Self {
        Self { specs: Mutex::new(VecDeque::new()) }
    }
}

impl<T: PartialEq> FaultQueue<T> {
    pub fn enqueue(&self, spec: FaultSpec<T>) {
        self.specs.lock().expect("fault queue lock poisoned").push_back(spec);
    }

    /// Returns the first outcome that matches `target`, in enqueue order.
    /// A `OneShot` entry is removed. A `Persistent` entry stays.
    pub fn take(&self, target: &T) -> Option<FaultOutcome> {
        let mut specs = self.specs.lock().expect("fault queue lock poisoned");
        let index = specs.iter().position(|spec| &spec.target == target)?;
        let outcome = specs[index].outcome.clone();
        if specs[index].scope == FaultScope::OneShot {
            specs.remove(index);
        }
        Some(outcome)
    }

    /// Returns the full spec when the test needs the message or the payload.
    pub fn take_spec(&self, target: &T) -> Option<FaultSpec<T>>
    where
        T: Clone,
    {
        let mut specs = self.specs.lock().expect("fault queue lock poisoned");
        let index = specs.iter().position(|spec| &spec.target == target)?;
        let spec = specs[index].clone();
        if spec.scope == FaultScope::OneShot {
            specs.remove(index);
        }
        Some(spec)
    }

    pub fn clear(&self) {
        self.specs.lock().expect("fault queue lock poisoned").clear();
    }

    pub fn is_empty(&self) -> bool {
        self.specs.lock().expect("fault queue lock poisoned").is_empty()
    }
}
```

Use `Mutex` and not `RefCell`, so the same fake works behind an `Arc` in async tests and in
tests that spawn threads.

## Target enum

Define one target enum per seam, with one variant per fallible operation. Keep the variants
close to the trait methods, so a test reads like the call it simulates.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportFault {
    Start,
    Send,
    Receive,
    Stop,
}
```

Do not use a string as the target. A typo in a string silently disables the fault and the
test passes for the wrong reason.

## A fake that consults the queue

The fake counts every call, records the last arguments, and asks the queue for an outcome
before it does its normal work.

```rust
pub trait Transport: Send + Sync {
    fn start(&self) -> Result<(), TransportError>;
    fn send(&self, frame: &[u8]) -> Result<usize, TransportError>;
}

#[derive(Default)]
pub struct FakeTransport {
    pub start_calls: AtomicUsize,
    pub send_calls: AtomicUsize,
    pub last_frame: Mutex<Option<Vec<u8>>>,
    pub faults: FaultQueue<TransportFault>,
}

impl Transport for FakeTransport {
    fn start(&self) -> Result<(), TransportError> {
        self.start_calls.fetch_add(1, Ordering::Relaxed);
        match self.faults.take(&TransportFault::Start) {
            Some(FaultOutcome::Error) => Err(TransportError::Io),
            Some(FaultOutcome::Timeout) => Err(TransportError::Timeout),
            Some(FaultOutcome::Panic) => panic!("simulated fault: transport start"),
            None => Ok(()),
            Some(other) => panic!("FakeTransport::start does not model {other:?}"),
        }
    }

    fn send(&self, frame: &[u8]) -> Result<usize, TransportError> {
        self.send_calls.fetch_add(1, Ordering::Relaxed);
        *self.last_frame.lock().expect("fake lock poisoned") = Some(frame.to_vec());
        match self.faults.take(&TransportFault::Send) {
            Some(FaultOutcome::Error) => Err(TransportError::Io),
            None => Ok(frame.len()),
            Some(other) => panic!("FakeTransport::send does not model {other:?}"),
        }
    }
}
```

Never end the match with `_ => Ok(..)`. The queue already removed the entry, so an outcome the
fake does not model passes as success, `faults.is_empty()` is true, and the test is a false
green. Panic on every outcome the fake does not model.

## Outcome selection

Match the outcome to the real failure mode of the seam. A test that injects the wrong
failure mode proves nothing about production behavior.

| Outcome | Simulates | Expected behavior under test |
|---|---|---|
| `Error` | An I/O or protocol error returned as `Err` | The caller maps it to its own error type and does not panic |
| `Timeout` | A peer that never answers | The caller times out, releases resources, and reports a timeout |
| `Drop` | A silently discarded message | The caller retries or reports loss, and does not hang forever |
| `Reset` | A connection reset by the peer | The caller tears down state and can be restarted |
| `MalformedPayload` | A well-formed call that returns corrupt bytes | The decoder returns `Err`, and does not panic or index out of bounds |
| `BlankPayload` | An empty but successful response | The caller treats empty as a valid, distinct case |
| `Panic` | A panic inside a callback or across a boundary | The panic is contained at the boundary, and the process does not abort |

`MalformedPayload` and `BlankPayload` are non-throwing outcomes. The call returns `Ok` with
bad data. They find a different class of defect than `Error`, because they exercise the
decode path instead of the error path.

Use `Panic` with care. It is the right tool to prove that a boundary contains a panic — an
FFI export, a thread body, a task. It is the wrong tool everywhere else. The
`rust-panic-safety` skill, when it is installed, has the containment rules.

## Scope rules

Default to `OneShot`.

Use `Persistent` only when the test exercises repeated calls to the same target, such as:

- A retry loop that must give up after N attempts.
- A polling loop that must report a permanent failure.
- A reconnect loop that must apply backoff.

A `Persistent` fault that is never cleared is a common source of confusing failures in
later tests, when a fake is accidentally shared. Two guards:

1. Create a fresh fake in every test.
2. Call `faults.clear()` in the teardown of any harness that outlives one test function.

## Asserting that the fault fired

An injected fault that never fires produces a green test that proves nothing. Assert both
sides:

```rust
#[test]
fn send_retries_once_after_a_transient_error() {
    let transport = Arc::new(FakeTransport::default());
    transport.faults.enqueue(FaultSpec {
        target: TransportFault::Send,
        outcome: FaultOutcome::Error,
        scope: FaultScope::OneShot,
        message: None,
        payload: None,
    });

    let service = Service::new(Arc::clone(&transport));
    service.send(b"frame").expect("the retry must succeed");

    // The fault fired, and the retry happened.
    assert!(transport.faults.is_empty(), "the injected fault never fired");
    assert_eq!(transport.send_calls.load(Ordering::Relaxed), 2);
}
```

`faults.is_empty()` proves only that every `OneShot` fault fired. A `Persistent` entry stays in
the queue whether it fired or not. For a `Persistent` fault, assert the call counter instead: a
retry loop that gives up after 3 attempts must leave `send_calls` at 3.

## A service-level fault test

The test enqueues one fault, drives the service through its public API, and asserts the mapped
error first, then the interaction:

```rust
use std::assert_matches;

#[test]
fn start_maps_transport_failure_to_service_error() {
    let transport = Arc::new(FakeTransport::default());
    transport.faults.enqueue(FaultSpec {
        target: TransportFault::Start,
        outcome: FaultOutcome::Error,
        scope: FaultScope::OneShot,
        message: Some("simulated start failure".to_owned()),
        payload: None,
    });

    let service = Service::new(Arc::clone(&transport));

    let error = service.start().expect_err("start must fail when the transport fails");
    assert_matches!(error, ServiceError::Transport { .. });
    assert_eq!(transport.start_calls.load(Ordering::Relaxed), 1);
}
```

`std::assert_matches!` (Rust 1.96) prints the value that did not match. It is not in the
prelude, so import it. Below MSRV 1.96, use `assert!(matches!(..))`, which prints only the
expression text.

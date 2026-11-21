# Host async model

Use this file when WebAssembly code awaits a host Promise, spawns a future,
uses Tokio or threads, or does async I/O on WASI.

## Use the host async model

An `async fn` does not create an executor. Select the bridge that the host
provides.

For browser and Node.js builds:

- Convert JavaScript `Promise` values with `wasm_bindgen_futures::JsFuture`.
- Export an `async fn` when JavaScript must receive a `Promise`.
- Use `wasm_bindgen_futures::spawn_local` for a detached `!Send` future.
- Never block the JavaScript event-loop thread with a polling loop, synchronous
  sleep, or native blocking I/O. A loop that waits for a Promise never ends,
  because no Promise can resolve while the thread is blocked. Other blocking
  calls freeze the page or the Node.js process.
- Do CPU-heavy work in bounded chunks or in a worker. An async wrapper alone
  does not move CPU work off the event-loop thread.

Do not enable a multi-thread Tokio runtime for ordinary browser WebAssembly.
Use only Tokio features that compile for the selected target, or use the host
Promise and timer APIs directly. WebAssembly threads require host support,
shared memory, worker setup, and the required HTTP isolation headers in a
browser. A successful `+atomics` compile proves none of those host conditions.

For WASI, do not assume a native epoll, kqueue, socket, or thread
implementation. Read `wasi.md` when you select the executor and I/O
library for a WASI preview.

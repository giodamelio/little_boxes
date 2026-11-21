---
description: Use when building, testing, or shrinking Rust for WebAssembly (wasm32-unknown-unknown, wasm32-wasip1, wasm32-wasip2, wasm32-wasip3, wasm32v1-none) in a browser, Node.js, a WASI runtime, or a WebAssembly Component Model host, or when diagnosing a wasm-bindgen boundary, instantiation, link, target-feature, or wasm binary size failure. Triggers on wasm-pack, wasm-bindgen, wit-bindgen, wasmtime, WASI, getrandom wasm_js, and "time not implemented on this platform".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-wasm
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 329cbec55824f0f0628887f236752f68fad176ce
name: rust-wasm
---
# Rust WebAssembly

Build for the exact WebAssembly host. A `.wasm` suffix does not define its ABI or imports.

## Write the host contract first

When you add a WebAssembly target, a host, or a published package, record the Rust target
triple, the host and its minimum version, the artifact format (core module or component), the
allowed imports and capabilities, the JavaScript output mode, and the engine feature baseline
(the features the oldest supported engine accepts). They decide the target, the dependencies,
and the test matrix.

Do not infer the host from `target_arch = "wasm32"`. Browser, Node.js, WASIp1,
WASIp2, and WASIp3 builds can share that architecture and still have incompatible APIs.

## Select the target

| Host contract | Rust target | Output | Important constraint |
|---|---|---|---|
| Browser or JavaScript host with explicit imports | `wasm32-unknown-unknown` | Core module, often with generated JS | `std::fs` fails, `println!` does nothing, `std::thread::spawn` panics, and `Instant::now()` and `SystemTime::now()` panic |
| Existing WASI Preview 1 host | `wasm32-wasip1` | Core module with `wasi_snapshot_preview1` imports | Use for compatibility; WASIp1 interfaces no longer grow |
| WASI 0.2 and Component Model host | `wasm32-wasip2` | Component | The host must support components and the WASI interfaces that the program imports |
| WASIp1 host with the threads proposal | `wasm32-wasip1-threads` | Shared-memory core module | The host must provide threads and shared memory |
| Minimal WebAssembly 1.0 engine with `no_std` | `wasm32v1-none` | Core module | Use only when `core` and `alloc` are sufficient |
| WASI 0.3 async component host | `wasm32-wasip3` | Component | Tier 2 since Rust 1.100.0; install `std` with rustup. Require WASI 0.3 async support in the host ([target page](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip3.html)) |

Do not use `wasm32-unknown-unknown` as a generic WASI target. Do not send a WASIp2 component to an API that accepts only
a core module.

The `std` targets do not emit WebAssembly 1.0 code. They enable `multivalue`,
`mutable-globals`, `reference-types`, and `sign-ext` by default, and `bulk-memory` and
`nontrapping-fptoint` since Rust 1.87 (LLVM 20). An engine without one of these
features can reject the module. For a 1.0 engine, use `wasm32v1-none` on stable,
or rebuild `std` for the MVP on nightly. Read `references/target-setup.md` when
the engine baseline is older than these defaults or the MSRV is below 1.87.

Do not enable a WebAssembly target feature only because the compiler accepts
it: the module then carries those instructions. Gate optional SIMD code with
`#[cfg(target_feature = "simd128")]` and keep a portable implementation.

Rust 1.84 removed `wasm32-wasi`: rename it to `wasm32-wasip1` in CI,
`.cargo/config.toml`, and `rust-toolchain.toml`.

## Triage

| Symptom | Likely cause | First check | Fix |
|---|---|---|---|
| `unknown import` at instantiation | Wrong target or missing host capability | Inspect imports with `wasm-tools print` | Select the correct target or implement and grant the import |
| `rust-lld: error: ...: undefined symbol: <name>` on Rust 1.96+ | The linker no longer passes `--allow-undefined`, so an undeclared import fails | Find the `extern` block or C object that should provide `<name>` | Put `#[link(wasm_import_module = "<module>")]` on a host import's `extern` block; fix the C build otherwise. `-Clink-arg=--allow-undefined` is only a migration shim |
| Wasmtime: `expected a WebAssembly module but was given a WebAssembly component`; V8 or Node.js: `expected version 01 00 00 00, found 0d 00 01 00` | Host accepts a core module, but the artifact is a component | Run `wasm-tools component wit` | Use a component host or build the required core-module target |
| `compile_error!` from `getrandom` on `wasm32-unknown-unknown` | No random backend for a JavaScript host | `cargo tree --locked --target wasm32-unknown-unknown -e features -i getrandom` | See "Keep portable logic separate" |
| "rust Wasm file schema version" from `wasm-bindgen` | CLI version differs from the locked crate | Compare `wasm-bindgen --version` with `Cargo.lock` | Install the exact locked version, or build through `wasm-pack` |
| `wasm-opt` rejects bulk-memory or non-trapping float-to-int instructions | The optimizer lacks the Rust 1.87+ default features | `found wasm-opt at ...` in the wasm-pack log, then `wasm-opt --version` | Enable both features in the `wasm-opt` arguments (`references/size-and-optimizer.md`) |
| `cc` build script fails for a WASI target, for example `'stdio.h' file not found` | `cc` fell back to a system `clang` with no WASI sysroot | `echo "$WASI_SDK_PATH"` and the `cc` version (1.2.39+) | Install wasi-sdk and set `WASI_SDK_PATH` (`references/wasi.md`) |
| Export is undefined | Wrong generated-JS mode, or initialization did not finish | The generated module | Match the consumer mode and await initialization |
| "time not implemented on this platform", or a bare `RuntimeError: unreachable` near timing code | `Instant::now()` or `SystemTime::now()` on `wasm32-unknown-unknown` | Install the panic hook in a debug build | Read the clock in the JavaScript adapter; for a dependency, enable its wasm feature or use `web-time` |
| `unreachable` or `RuntimeError` after a Rust failure | Panic became a trap | Enable bounded panic diagnostics and inspect the first panic | Return a typed error for expected failure; fix the panic; discard the instance when state integrity matters |
| Callback fails after setup returns | Its Rust `Closure` was dropped | Find the owner and unregister path | Store the closure until listener removal |
| View returns corrupt or old bytes | Linear memory grew | Compare `memory.buffer` before and after allocation | Recreate the typed-array view |
| Browser passes but Node.js fails | DOM or ES-module assumption reached Node.js | Run the generated Node.js package test | Move the API behind the correct host adapter |
| Host build passes but Wasm target fails | Unconditional OS or thread dependency | `cargo tree --target <triple> -e features` | Gate the dependency behind a portable interface |
| Module validates but fails on the oldest engine | Default post-1.0 features, an enabled proposal, or a WIT version the host lacks | `wasm-tools validate --features=wasm1` (or `wasm2`) | Build for the baseline or raise the documented minimum |
| Size jumps after a small API change | Generic duplication, extra `web-sys` features, or glue growth | Compare `twiggy top` or `wasm-tools objdump` output and package contents | Remove the measured source of growth |

## Verify the matrix that ships

Pass all applicable layers: portable logic on the host (`cargo nextest run --locked`, or
`cargo test --locked`); every supported target and feature set compiled; boundary tests in each
real host mode; and the final packaged artifact loaded from the package or deployment directory,
not from Cargo's target directory.

```bash
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
wasm-pack test --node
```

Do not replace browser tests with Node.js tests: Node.js has no DOM and uses a
different module loader. `cargo check --target` does not instantiate the module
or resolve imports.

Add one JavaScript integration test that imports the generated package, awaits
initialization, calls a success path, and verifies the public error shape and a
64-bit value round trip. It catches stale glue and a missing `.wasm` asset.

Build an explicit CI matrix of target, Cargo features, host, and minimum host
version. `cargo tree --locked --target <triple> -e features` finds a feature that
leaks an unsupported dependency into a row.

Validate each final artifact against the engine baseline with
`wasm-tools validate --features=wasm2` (or `wasm1`), and compare its
`wasm-tools print` import list with the host contract. Plain
`wasm-tools validate` accepts every phase 4 proposal, so it proves nothing about
an old engine. Read `references/target-setup.md` when you write the validate
command: it has the SIMD and component-model variants.

Read `references/wasi.md` when a matrix row targets `wasm32-wasip1` or
`wasm32-wasip2` or `wasm32-wasip3`. Grant a WASI run only the capabilities that it needs: a run with
broad inherited access does not prove the production policy. Pin the WIT package
versions and the host runtime together.

Run the checks again after a toolchain, dependency, or `wasm-opt` change.

Measure the final packaged `.wasm`, raw and gzip-compressed, against a size
budget. Pin the wasm-pack version: an upgrade can change the bundled `wasm-opt`. Read
`references/size-and-optimizer.md` when you set a size budget, change the
release profile, or change how `wasm-opt` runs.

## Completion evidence

Report the host contract and capability policy, the Cargo feature set and engine baseline, the
build, baseline-validation, and runtime commands and import list of every shipped matrix row, a
successful import of the final JavaScript package, raw and compressed sizes against the budget,
and every row that could not run.

## Keep portable logic separate

Put business logic in a normal Rust library and the browser, Node.js, and WASI
adapters at the edge.

Use target predicates only for facts the compiler knows:
`cfg(all(target_family = "wasm", target_os = "unknown"))` for a bare JavaScript
host. For WASI, pair `target_os = "wasi"` with `target_env = "p1"`, `"p2"`, or `"p3"`.
`cfg` cannot tell a browser from Node.js: select adapters with explicit Cargo
features and reject invalid sets.

```rust
#[cfg(all(feature = "browser", feature = "node"))]
compile_error!("select only one JavaScript host feature");

fn main() {}
```

Put browser-only crates in a `[target.'cfg(all(target_family = "wasm", target_os = "unknown"))'.dependencies]`
table so they stay out of WASI resolution, and enable only the `web-sys` interfaces the code uses.
Inject the clock: on `wasm32-unknown-unknown`, read time in the JavaScript adapter, because
`std::time` panics there. Read `references/javascript-host.md` when you write that table or the
clock adapter.

`getrandom` has no default backend on `wasm32-unknown-unknown`. Enable feature
`wasm_js` (0.3.4 and later, 0.4) or `js` (0.2) for each major version in the
graph. Set it in the final `cdylib` or binary crate, not in a library: upstream
warns that it breaks non-JavaScript WebAssembly builds. Read
`references/target-setup.md` when the graph holds `getrandom` 0.3.0 to 0.3.3 or
more than one major version.

## Read the topic references

| Task | Read |
| --- | --- |
| Export to a browser or Node.js through `wasm-bindgen`: crate type, `wasm-pack` output modes, CLI version rule, initialization; or design exports, imports, callbacks, and error values across the JavaScript boundary | `references/javascript-host.md` |
| Choose the panic strategy, install a panic hook, or decide whether an instance survives a trap | `references/panic-and-errors.md` |
| Await a JavaScript `Promise`, spawn a future, use Tokio or threads, or do async I/O on WASI | `references/async-hosts.md` |
| Ship a WASI component, or compile C or C++ for a WASI target | `references/wasi.md` |

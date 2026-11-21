# WASI builds and components

Use this file when you build for `wasm32-wasip1`, `wasm32-wasip2`, or `wasm32-wasip3`, select
async I/O for WASI, ship a WebAssembly component, or compile C or C++ for a
WASI target. The capability
and WIT pinning rules in `SKILL.md` apply to every command here.

Contents: Preview 1; WASI 0.2; WASI 0.3 and its async/capability smoke test;
async I/O selection; C and C++ builds.

## Build and run Preview 1

Build Preview 1 only for a host that implements its imports:

```bash
cargo build --locked --release --target wasm32-wasip1
wasm-tools validate target/wasm32-wasip1/release/<binary>.wasm
wasmtime run --dir ./fixtures target/wasm32-wasip1/release/<binary>.wasm
```

## Build and run a WASI 0.2 component

Build a WASI 0.2 component for a component-aware host:

```bash
cargo build --locked --release --target wasm32-wasip2
wasm-tools validate target/wasm32-wasip2/release/<binary>.wasm
wasm-tools component wit target/wasm32-wasip2/release/<binary>.wasm
wasmtime run target/wasm32-wasip2/release/<binary>.wasm
```

Use native `wasm32-wasip2` and `wit-bindgen` tooling for new components. Plain
`cargo build --target wasm32-wasip2` produces a component (Rust 1.82 and later).
Do not introduce `cargo-component` without a project-specific requirement. Its
README (0.21.1, as of 2026-10) calls it experimental, and a newer release can
break the build of an existing project. The same README still points to
`cargo component` for a project with non-WASI WIT interfaces.

## Build a WASI 0.3 component

Rust 1.100 promotes `wasm32-wasip3` to Tier 2. Install the target with
`rustup target add wasm32-wasip3`, then run
`cargo build --locked --release --target wasm32-wasip3`.
Inspect the component imports with `wasm-tools component wit` and run the result
in the shipping host with WASI 0.3 async support. A build does not prove host support.
The Rust 1.100 target does not yet enable cooperative `std::thread` support by default.
Use `cfg(all(target_os = "wasi", target_env = "p3"))` for target-specific code.
Use wasi-sdk 34 or later when an external C toolchain is required.

### Exercise the async host and capability boundary

This smoke test is checked with Rust 1.100.0, Wasmtime 50.0.1, and wasm-tools
1.263.0. Pin the runtime and tooling before repeating it. A Tier 2 compiler
target does not establish a host runtime's support or stability policy.

```rust
// guest.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_millis(25));
    assert!(start.elapsed() >= std::time::Duration::from_millis(25));
    println!("async clock wait completed");
    let data = std::fs::read_to_string("/fixture/input.txt")?;
    assert_eq!(data, "capability granted\n");
    println!("file capability granted");
    Ok(())
}
```

```bash
rustup target add --toolchain 1.100.0 wasm32-wasip3
rustc +1.100.0 --edition=2024 --target wasm32-wasip3 guest.rs -o guest.wasm
wasm-tools validate guest.wasm
wasm-tools component wit guest.wasm > imports.wit
mkdir -p fixtures
printf 'capability granted\n' > fixtures/input.txt
wasmtime run -S p3 -W component-model-async --dir ./fixtures::/fixture guest.wasm
```

Check `imports.wit`: the command exports `wasi:cli/run@0.3.0` with an async
`run`, imports `wasi:clocks/monotonic-clock@0.3.0` with async waits, and imports
the WASI 0.3 filesystem. The Rust call is synchronous at its source boundary;
the component uses async host interfaces. It does not spawn an OS thread.

The run prints both completion lines. Repeat it without `--dir`:

```bash
wasmtime run -S p3 -W component-model-async guest.wasm
```

Require a nonzero exit, the clock completion line, and the file error. Wasmtime
50.0.1 reports `NotFound` for this ungranted path. Also run with `-S p3=n` and
require a missing `wasi:cli/environment@0.3.0` linker implementation. These
negative cases prevent an unrelated startup failure from passing as a capability
test.

This proves a clock wait and one file capability on the pinned host. It does not
prove cancellation, HTTP streaming, cooperative threads, or another runtime.
Grant only the directory under test; avoid `--dir /` and inherited network access
when checking this boundary.

Sources: [Rust WASIp3 target](https://doc.rust-lang.org/1.100.0/rustc/platform-support/wasm32-wasip3.html),
[Wasmtime CLI](https://docs.wasmtime.dev/cli-options.html),
[Wasmtime 50.0.1](https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.1).

## Select async I/O for WASI

Select the executor and I/O library that supports the exact WASI preview and
runtime. Do not assume a native epoll, kqueue, socket, or thread implementation
exists. Run the I/O path under the shipping runtime.

## Compile C or C++ for WASI

Rust links WASI programs against its bundled `wasi-libc`, so pure Rust needs no
C toolchain. When a build script compiles C or C++ through the `cc` crate,
install wasi-sdk and set `WASI_SDK_PATH` to its directory. `cc` then uses
`$WASI_SDK_PATH/bin/<target>-clang`, which already selects the WASI sysroot.

This needs `cc` 1.2.39 or later. An older `cc` ignores `WASI_SDK_PATH` and runs
the system `clang`, which fails with errors such as `'stdio.h' file not found`.
Run `cargo update -p cc`, or set `CC_<target>`, for example
`CC_wasm32_wasip2=$WASI_SDK_PATH/bin/wasm32-wasip2-clang`.

For C++, also set `WASI_SYSROOT=$WASI_SDK_PATH/share/wasi-sysroot`. `cc` links
libc++ and libc++abi only when it is set, and Rust 1.96+ makes the missing C++
runtime symbols a link error.

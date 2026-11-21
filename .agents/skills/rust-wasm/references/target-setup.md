# Target setup details

Use this file when the engine baseline is older than the Rust defaults, when
the MSRV is below Rust 1.87, when you validate an artifact against the engine
baseline, or when `getrandom` fails to build for `wasm32-unknown-unknown`.

## Default WebAssembly features by Rust version

The `std` targets do not emit WebAssembly 1.0 code. They enable these features
by default
([target page](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html)):

| Rust version | Features enabled by default from this version |
|---|---|
| 1.70 | `sign-ext`, `mutable-globals` |
| 1.82 | `multivalue`, `reference-types` |
| 1.87 | `bulk-memory`, `nontrapping-fptoint` |

A toolchain bump can add a feature that the oldest supported engine rejects.
Install the targets and inspect the features that the current toolchain
enables:

```bash
rustup target add wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2
rustc --print cfg --target wasm32-wasip2
rustc -Ctarget-feature=help --target wasm32-unknown-unknown
```

## Build `std` for a WebAssembly 1.0 engine

On stable, use `wasm32v1-none` (`no_std`, `core` and `alloc` only). When the
code needs `std`, rebuild it for the MVP feature set on nightly:

```bash
rustup component add rust-src --toolchain nightly
RUSTFLAGS=-Ctarget-cpu=mvp cargo +nightly build -Zbuild-std=panic_abort,std --target wasm32-unknown-unknown
```

Validate the result with `wasm-tools validate --features=wasm1`.

## Validate against the engine baseline

`wasm2` includes SIMD; append `,-simd` when the baseline has no SIMD. A `wasm1`
or `wasm2` group replaces the whole feature set and turns off the component
model, so for a component add `,component-model`.

```bash
wasm-tools validate --features=wasm2 pkg/<name>_bg.wasm   # WebAssembly 2.0 engine
wasm-tools validate --features=wasm1 pkg/<name>_bg.wasm   # WebAssembly 1.0 engine
wasm-tools validate --features=wasm2,component-model target/wasm32-wasip2/release/<binary>.wasm
wasm-tools print pkg/<name>_bg.wasm | grep '(import '     # compare with the host contract
```

## Configure `getrandom` for `wasm32-unknown-unknown`

`getrandom` has no default backend on `wasm32-unknown-unknown`, so the build
fails with a `compile_error!` from `getrandom`. The fix depends on the major
version. Several majors can be in one graph.

| `getrandom` version | Setting for a JavaScript host |
|---|---|
| 0.2 | Feature `js` |
| 0.3.0 to 0.3.3 | Run `cargo update -p getrandom@<0.3.x>` to reach 0.3.4 or later, then use feature `wasm_js` alone. If you cannot update, add feature `wasm_js` and set `--cfg getrandom_backend="wasm_js"` under `[target.wasm32-unknown-unknown] rustflags` in `.cargo/config.toml` |
| 0.3.4 and later, 0.4 | Feature `wasm_js` |

```bash
cargo tree --locked --target wasm32-unknown-unknown -e features -i getrandom
```

If `cargo tree` reports that the spec is ambiguous, run it again with each
`getrandom@<version>` that it lists. Add a direct target-specific dependency on
each major version with its feature, and use `package = "getrandom"` to rename a
second major. Set the feature in the final `cdylib` or binary crate, not in a
library: upstream warns that it breaks non-JavaScript WebAssembly builds.

# The `*-sys` crate and a deterministic `build.rs`

Read this when you create or change a `foo-sys` package, publish
`cargo::metadata`, or write the change-detection rules of `build.rs`.

Contents:

- Package layout
- Metadata for dependents
- Change detection
- Authoritative references

## Package layout

A `foo-sys` package declares `links = "foo"`, discovers or builds `libfoo` in
`build.rs`, emits the link instructions once, and holds the raw declarations
or generated bindings. Keep the safe API in a separate crate. Do not put
business logic in `foo-sys`.

```toml
[package]
name = "foo-sys"
version = "0.1.0"
edition = "2024"
links = "foo"
build = "build.rs"

[build-dependencies]
# Add only the helper selected by the integration path.
```

## Metadata for dependents

Publish include paths or other facts after discovery, when an immediate
dependent build script needs them:

```text
cargo::metadata=include=/absolute/target/include
cargo::metadata=version=1.2.3
```

An immediate dependent reads these values as `DEP_FOO_INCLUDE` and
`DEP_FOO_VERSION`. Cargo does not pass `DEP_*` metadata through transitive
dependencies. Forward a value deliberately if another native layer needs it.

## Change detection

Emit one change rule for every direct file and external environment variable
that the script reads. A directory rule scans the full directory. Prefer file
rules when the input set is known.

```rust
use std::env;
use std::path::PathBuf;

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Cargo did not set {name}"))
}

fn main() {
    println!("cargo::rerun-if-changed=native/wrapper.h");
    println!("cargo::rerun-if-changed=native/source.c");
    println!("cargo::rerun-if-env-changed=FOO_ROOT");

    let target = required("TARGET");
    let target_os = required("CARGO_CFG_TARGET_OS");
    let out_dir = PathBuf::from(required("OUT_DIR"));

    eprintln!("native target={target} os={target_os} out={}", out_dir.display());
}
```

Do not emit `rerun-if-env-changed` for `TARGET` or another variable that Cargo
sets for the build script. Emit it for external inputs such as `FOO_ROOT`,
`CC`, or a custom SDK path when a selected helper does not already track it.

If the script has no external input, emit
`cargo::rerun-if-changed=build.rs`. Without any `rerun-if` instruction, Cargo
scans the package and can run the script after any package file changes.

Use `cargo::KEY=VALUE` on Rust 1.77 or newer. Use the legacy
`cargo:KEY=VALUE` spelling only when the declared MSRV is older than 1.77.

## Authoritative references

- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Cargo build-script environment](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts)

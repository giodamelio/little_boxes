# Link instructions and archive order

Read this when you write `cargo::rustc-link-*` instructions, fix an archive
order defect, or set up the `--warn-backrefs` CI lane.

Contents:

- Structured instructions
- Archive order
- The `--warn-backrefs` lane
- Raw linker arguments
- Authoritative references

## Structured instructions

Prefer structured instructions over raw linker arguments:

```text
cargo::rustc-link-search=native=/absolute/target/lib
cargo::rustc-link-lib=static=foo
cargo::rustc-link-lib=dylib=bar
cargo::rustc-link-search=framework=/absolute/Frameworks
cargo::rustc-link-lib=framework=CoreFoundation
```

The complete library syntax is
`[KIND[:MODIFIERS]=]NAME[:RENAME]`. Use `static`, `dylib`, or `framework` as
the kind. Add a modifier such as `+whole-archive` only when inspection proves
that normal archive extraction omits required registration objects.

Do not duplicate instructions that `cc`, `pkg-config`, `vcpkg`, or another
helper already emits. Configure the helper not to emit metadata only when the
script must control the final order itself.

## Archive order

The order of printed instructions can become linker argument order. Emit a
consumer object or archive before the libraries that satisfy its undefined
symbols. If native archive `foo` calls `bar`, emit `foo` before `bar` on a
one-pass linker. Fix the order before adding `--start-group` or
`+whole-archive`; both can hide a dependency cycle and increase the artifact.

## The `--warn-backrefs` lane

`lld` resolves a backward archive reference that GNU ld rejects. Since Rust
1.90, the rustup toolchain on an x86_64 Linux host links
`x86_64-unknown-linux-gnu` with `rust-lld` by default. An order defect passes
there and fails on a GNU ld target. Expose it in a dedicated CI lane on an
x86_64 Linux runner with the rustup toolchain:

```bash
RUSTFLAGS="-C link-arg=-Wl,--warn-backrefs -D linker-messages" \
  cargo test --locked --no-run --all-targets --target x86_64-unknown-linux-gnu
```

A defect prints `backward reference detected: <symbol> in <consumer archive>
refers to <provider archive>`. `--warn-backrefs` only warns through the
`linker_messages` lint, so `-D linker-messages` makes the lane fail. The lane
must link an artifact that uses the native archives. `cargo build` of a
`*-sys` library makes an rlib and runs no linker; a test that calls one native
function supplies the link. `RUSTFLAGS` replaces the rustflags from config
files, so keep it out of the normal build.

To get a hard GNU ld failure instead, pass `-C linker-features=-lld` on the
same runner, or build on an `aarch64-unknown-linux-gnu` runner. Both link with
the system linker.

## Raw linker arguments

Use `cargo::rustc-link-arg-*` only when Cargo has no structured instruction.
Select the narrow target type:

```text
cargo::rustc-link-arg-cdylib=-Wl,<platform-option>
cargo::rustc-link-arg-bin=app=-Wl,<platform-option>
```

These instructions affect only targets in the package whose `build.rs` emits
them. A `foo-sys` build script cannot add a run path to a dependent
application or `cdylib`. Put final-artifact linker arguments in the package
that builds that artifact, or in its host build system.

## Authoritative references

- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Rust native link attribute](https://doc.rust-lang.org/reference/items/external-blocks.html#the-link-attribute)
- [rust-lld default on x86_64 Linux](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable/)
- [lld `--warn-backrefs`](https://lld.llvm.org/ELF/warn_backrefs.html)

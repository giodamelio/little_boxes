---
description: Use when cross-compiling Rust for another Linux architecture or building a static binary - rustup target add, a target linker and CC_<target> for cc build scripts, x86_64-unknown-linux-musl, crt-static, cross, cargo-zigbuild with a glibc version suffix, Docker multi-stage builds, and qemu-user tests. Triggers on "linking with `cc` failed" and "version `GLIBC_2.xx' not found". Not for Android, iOS, wasm, or no_std; use rust-android-build, rust-ios-build, rust-wasm, or rust-embedded-no-std.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-cross-compilation
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: cb3ddbeacbff154ee76eece31640f3715fdbf8a3
name: rust-cross-compilation
---
# Rust Cross-Compilation

A foreign target needs three things, and `rustup` installs only the first:

1. the standard library for the triple (`rustup target add <triple>`);
2. a linker for the target and its C library;
3. a C compiler, archiver, and sysroot for each build script that compiles C.

Related skills, when installed: `cargo-workflows` owns where `.cargo/config.toml` lives and the
`RUSTFLAGS` precedence. `rust-native-linking` owns `build.rs`, `*-sys` crates, and `HOST` against
`TARGET` in a build script. `rust-android-build`, `rust-ios-build`, `rust-wasm`, and
`rust-embedded-no-std` own their platforms.

## Gotchas

- The default linker of every Linux target is the host `cc`. On Rust 1.99.1, an
  `aarch64-unknown-linux-gnu` build on an x86_64 host with no linker configured stops with
  ``linking with `cc` failed`` and `rust-lld: error: --fix-cortex-a53-843419 is only supported on
  AArch64` (GNU ld: `unrecognized option '--fix-cortex-a53-843419'`).
- A plain `CC` applies to the target too. `cc` 1.11.1 reads `CC_<triple>`, then
  `CC_<triple_with_underscores>`, then `TARGET_CC`, then `CC`. With `CC=cc` exported, the build
  scripts of `ring` 0.17.14 and `libmimalloc-sys` 0.1.49 compiled x86-64 objects into an aarch64
  build without an error; only the final link failed. Set the target-qualified name, never a bare
  `CC`, in a cross job.
- With no variable set, `cc` 1.11.1 derives the compiler from the triple, for example
  `aarch64-linux-gnu-gcc`, or `x86_64-linux-musl-gcc` then `musl-gcc`. A linker named
  `<prefix>-gcc` also sets the prefix.
- `x86_64-unknown-linux-musl` is a cross target on an x86_64 glibc host: `pkg-config` refuses it,
  and `cc` needs a musl compiler.
- `RUSTFLAGS="-C target-feature=+crt-static"` without `--target` also applies to build scripts and
  proc macros. Rust 1.99.1 then stops with `cannot produce proc-macro for <crate> as the target
  x86_64-unknown-linux-gnu does not support these crate types`. Always pass `--target` with this
  flag.
- A glibc binary needs the newest `GLIBC_x.y` symbol version it links, and the build machine's
  glibc decides that. A hello world built by Rust 1.99.1 on Ubuntu 24.04 (glibc 2.39) needs
  `GLIBC_2.34`. Rust 1.99.1 supports glibc 2.17 and later on the x86_64 and aarch64 GNU targets.
- `ldd` runs the host dynamic loader. It cannot read a foreign-architecture binary. Use `readelf`.

## Triage

| Symptom | Cause | Fix |
|---------|-------|-----|
| `` error[E0463]: can't find crate for `std` `` and `the <triple> target may not be installed` | No standard library for the triple | `rustup target add <triple>` for the toolchain that builds; add it to `targets` in `rust-toolchain.toml` |
| ``linking with `cc` failed`` on a foreign target | Host `cc` links the target objects | Set `linker` for the target (below) and install the cross GCC |
| ``linker `<name>` not found`` | The configured linker is not on `PATH` | Install the toolchain, or correct the name |
| `failed to find tool "aarch64-linux-gnu-gcc"` or `"x86_64-linux-musl-gcc"` from `cc-rs` | No C cross compiler for a build script | Install it, or set `CC_<triple>` and `AR_<triple>` |
| Build scripts pass, the link fails with undefined or incompatible objects | A bare `CC` or `AR` picked the host tools | Unset the bare variables; set the `_<triple>` forms |
| `openssl-sys`: `Could not find directory of OpenSSL installation` and `pkg-config has not been configured to support cross-compilation` | No target OpenSSL, or `pkg-config` blocks the cross build | See "OpenSSL, ring, and aws-lc" below |
| ``version `GLIBC_2.xx' not found (required by <binary>)`` at start | Built against a newer glibc than the run host has | Build on an older base image, or use `cargo zigbuild --target <triple>.2.17`, or ship musl |
| GNU ld warns `Using 'getaddrinfo' in statically linked applications` | Static glibc still loads NSS shared libraries at run time | Use musl for a fully static binary |
| musl binary is many times slower under threads | musl `malloc` under contention | Set a global allocator and measure again |
| `cannot produce proc-macro ... does not support these crate types` | `crt-static` in `RUSTFLAGS` without `--target` | Pass `--target` |
| Tests build, then `Exec format error` (os error 8) | No runner or binfmt entry for the foreign binary | Configure `runner` (below), or report a compile-only lane |

## Configure a target

Put the per-target tools in `.cargo/config.toml` at the invocation root. Use the bare tool name,
not an absolute path.

```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
runner = ["qemu-aarch64", "-L", "/usr/aarch64-linux-gnu"]
```

In CI, the same values come from the environment. Cargo reads `CARGO_TARGET_<TRIPLE>_LINKER` and
`CARGO_TARGET_<TRIPLE>_RUNNER` (triple upper-cased, `-` as `_`); `cc` reads the C tools:

```bash
rustup target add aarch64-unknown-linux-gnu
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
cargo build --locked --release --target aarch64-unknown-linux-gnu
```

Set the linker and the C compiler from one toolchain. Otherwise Rust code links against one C
library and the bundled C code compiles against another.

On Ubuntu 24.04, these packages hold the tools (checked on packages.ubuntu.com, 2026-11-12):

| Need | Package | Tool |
|------|---------|------|
| aarch64 glibc linker and C compiler | `gcc-aarch64-linux-gnu` | `aarch64-linux-gnu-gcc`; the loader lands in `/usr/aarch64-linux-gnu/lib` |
| musl C compiler for x86_64 | `musl-tools` | `musl-gcc`, which `cc` 1.11.1 finds for `x86_64-unknown-linux-musl` |
| User-mode emulation | `qemu-user` | `qemu-aarch64` |
| Run foreign binaries directly | `qemu-user-binfmt` | binfmt_misc entries |

## Static Linux binaries with musl

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --locked --release --target x86_64-unknown-linux-musl
file target/x86_64-unknown-linux-musl/release/<binary>
```

The musl targets link statically by default. On Rust 1.99.1 (musl 1.2.5), `file` reports
`static-pie linked`, and `readelf -d` lists no `NEEDED` entry. A pure-Rust crate graph needs no
C toolchain for this.

**C dependencies.** Each `cc`-based build script needs a musl compiler. On x86_64, install
`musl-tools`; `cc` then finds `musl-gcc`. For `aarch64-unknown-linux-musl`, `cc` looks for
`aarch64-linux-musl-gcc`, which Ubuntu does not package; use `cross` or `cargo-zigbuild`.

**OpenSSL, ring, and aws-lc.** `openssl-sys` 0.9.118 finds no OpenSSL for a musl target and
fails. Pick one fix and record it:

- Enable the `vendored` feature of `openssl` (0.10.82): `openssl-src` builds OpenSSL from source
  with the target C compiler.
- Point `<TRIPLE>_OPENSSL_DIR` (for example `X86_64_UNKNOWN_LINUX_MUSL_OPENSSL_DIR`) at an OpenSSL
  built for the target.
- Move TLS to `rustls`. Its providers still compile C and assembly: `ring` 0.17.14 and
  `aws-lc-sys` 0.47.0 both stop with `failed to find tool "x86_64-linux-musl-gcc"` until a musl
  compiler exists.

**Allocator.** musl `malloc` is slow under thread contention. Measured with Rust 1.99.1 on a
4-CPU x86_64 host, 8 threads that each make 2 million allocations of 16 to 527 bytes: 77 to 81 ms
on `x86_64-unknown-linux-gnu`, 6.5 to 8.2 s on `x86_64-unknown-linux-musl`. For a multi-threaded
service, set a global allocator in the binary crate, for example
`#[global_allocator] static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;` with
`mimalloc = "0.1.52"`, or `tikv-jemallocator` 0.7.0. Both compile C, so they need the musl
compiler too. Benchmark the real workload on both targets before and after.

**Static glibc instead.** `RUSTFLAGS="-C target-feature=+crt-static" cargo build --target
x86_64-unknown-linux-gnu` gives a `static-pie` glibc binary. glibc still loads NSS modules at run
time for `getaddrinfo`. GNU ld warns `Using 'getaddrinfo' in statically linked
applications requires at runtime the shared libraries from the glibc version used for linking`;
the default linker of Rust 1.99.1 prints nothing. Prefer musl when "static" must mean "runs on
any Linux".

## Pick a build environment

| Approach | Use when | Main cost |
|----------|----------|-----------|
| Distribution cross GCC on the host | One or two GNU targets, C deps from the distribution | glibc floor equals the host's glibc |
| `cross` (containers) | Many targets, C deps, and tests under QEMU in one command | Needs Docker or Podman; image pinning |
| `cargo-zigbuild` | A glibc floor below the build host's, without containers | No static glibc; Zig's own headers |
| Docker multi-stage image | The deliverable is a container image | Image tags and base glibc to pin |

Read [references/containers-and-zig.md](references/containers-and-zig.md) to set up
`cross`, `cargo-zigbuild`, or a multi-stage Dockerfile, or when a glibc floor must hold.

## Run foreign-architecture tests

`cargo test --target <triple>` runs each test binary through `runner`. On Rust 1.99.1 doctests go
through the same runner; mark a doctest that cannot run on a target with an `ignore-<target-substring>`
fence attribute such as `ignore-aarch64` (Rust 1.88 and later). With `qemu-user` installed, the runner from "Configure a target" runs an
aarch64 glibc binary; `-L` (or `QEMU_LD_PREFIX`) names the directory that holds the target
loader. A static musl binary needs no `-L`. With `qemu-user-binfmt`, the kernel starts foreign
binaries directly, but a glibc binary still needs `QEMU_LD_PREFIX`.

qemu-user emulates the CPU only; the host kernel serves the system calls. A pass under qemu-user
does not prove behavior on the real kernel or hardware. When no runner exists, build the tests
and report the lane as compile-only, never as a pass:

```bash
cargo test --locked --no-run --target aarch64-unknown-linux-gnu
```

## Verify the artifact

Check the file that ships, not a build from another job.

| Claim | Command | Green does not prove |
|-------|---------|----------------------|
| Right architecture | `file <binary>` (`ARM aarch64`, `x86-64`) or `llvm-readelf -h <binary>` (`Machine`) | That it runs |
| Fully static | `file` says `static-pie linked` or `statically linked`; `llvm-readelf --needed-libs <binary>` lists nothing | That NSS or `dlopen` is unused |
| Shared libraries it needs | `readelf -d <binary> \| grep NEEDED` and `readelf -lW <binary> \| grep interpreter` | That the run host has them |
| glibc floor | `readelf -W --version-info <binary> \| grep -o 'Name: GLIBC_[0-9.]*' \| sort -uV \| tail -1` | Kernel or CPU feature floor |
| It starts and passes tests | The runner, or the oldest supported host | Behavior on real hardware, under qemu |

Run the glibc-floor check before `strip`; it reads the dynamic symbol versions.

## Completion evidence

Report the triple, the toolchain version, the linker and C compiler per target, the build
environment (host, `cross` image, Zig version, or base image), the `file` and `NEEDED` output of
each shipped binary, its glibc floor, and the test lane: native, qemu-user, or compile-only.

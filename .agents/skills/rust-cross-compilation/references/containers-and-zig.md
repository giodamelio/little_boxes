# cross, cargo-zigbuild, and Docker Images

Read this file to set up `cross`, `cargo-zigbuild`, or a multi-stage Dockerfile, or when a
binary must run on an older glibc than the build host has.

Contents:

- Hold a glibc floor
- cross
- cargo-zigbuild
- Multi-stage Docker image

## Hold a glibc floor

A `*-linux-gnu` binary needs the highest `GLIBC_x.y` symbol version it references. The loader
reports a miss at start in the form `<binary>: <libc path>: version `GLIBC_2.34' not found
(required by <binary>)`. Rust 1.99.1 supports glibc 2.17 and later on the
tier 1 Linux GNU targets, so the floor comes from the build environment, not from Rust.

Hold the floor with one of these:

1. Build in an image whose glibc is at or below the floor.
2. Build with `cargo zigbuild --target <triple>.<glibc>`.
3. Ship a `*-linux-musl` binary, which has no glibc floor.

Add a CI gate that reads the floor of each shipped binary and fails above the documented value:

```bash
max=$(readelf -W --version-info "$bin" | grep -o 'Name: GLIBC_[0-9.]*' \
  | sed 's/Name: GLIBC_//' | sort -uV | tail -1)
printf '%s\n%s\n' "$max" "$floor" | sort -V -C || { echo "needs glibc $max > $floor"; exit 1; }
```

Run the gate before `strip`. Then start the binary once on the oldest supported distribution. The
gate proves the symbol versions only, not the kernel or CPU baseline.

## cross

`cross` runs cargo inside a per-target container that holds the linker, the C toolchain, a
sysroot, and QEMU. Its CLI matches cargo's:

```bash
cross build --locked --release --target aarch64-unknown-linux-gnu
cross test --locked --target aarch64-unknown-linux-gnu
```

Facts as of 2026-11-12:

- crates.io holds `cross` 0.2.5. Its README installs from git:
  `cargo install cross --git https://github.com/cross-rs/cross`. Pin `--rev <sha>` in CI so the
  tool and its default images do not move.
- It needs Docker (1.24 or later) or Podman (1.6.3 or later). With both installed it uses Docker;
  `CROSS_CONTAINER_ENGINE=podman` overrides that.
- The images are `ghcr.io/cross-rs/<triple>:<tag>`. In 0.2.5 the `aarch64-unknown-linux-gnu`
  image starts `FROM ubuntu:16.04`, sets the linker to `aarch64-linux-gnu-gcc`, and sets
  `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER` to a QEMU wrapper. `cross test` therefore runs
  under qemu-user; the qemu-user limits of `SKILL.md` apply.

Configure it in `Cross.toml` at the project root (or `[package.metadata.cross]` in
`Cargo.toml`):

```toml
[build.env]
passthrough = ["RUST_LOG"]

[target.aarch64-unknown-linux-gnu]
pre-build = ["dpkg --add-architecture arm64 && apt-get update && apt-get install --assume-yes libssl-dev:arm64"]
```

`pre-build` adds target libraries to a derived image. `image = "<name>:<tag>"` replaces the image;
base a custom image on the default one for the same triple. `passthrough` lists the host
environment variables to pass into the container.

## cargo-zigbuild

`cargo-zigbuild` uses `zig cc` as the linker and C compiler. Zig ships its own glibc stubs and
headers, so the build host's glibc does not set the floor.

```bash
cargo install --locked cargo-zigbuild    # 0.24.1 as of 2026-11-12, rust-version 1.88
pip3 install ziglang                     # or install zig from ziglang.org
rustup target add aarch64-unknown-linux-gnu
cargo zigbuild --locked --release --target aarch64-unknown-linux-gnu.2.17
```

The `.2.17` suffix sets the glibc floor; the artifact still goes to
`target/aarch64-unknown-linux-gnu/`. Rules from the 0.24.1 README:

- Always pass the suffix. Without it, Zig picks a default glibc that changes with the Zig
  release.
- Without `--target`, Zig is not used, and the command is a plain `cargo build`.
- An invalid glibc version falls back silently; read the floor with the gate above.
- `-C target-feature=+crt-static` on a glibc target is not supported. Use a `*-musl` target for a
  static binary.
- `zig cc` uses `-nostdinc`, so `/usr/include` is not searched. When a C dependency needs a header
  from a system package, set `CFLAGS='-isystem /usr/include'`; `CPATH=/usr/include` mixes the
  host glibc headers with Zig's and fails.
- Build scripts see the resolved Zig target in `CARGO_ZIGBUILD_TARGET`.

Pin the Zig version next to the Rust toolchain. A Zig update changes the C compiler of every
build script.

## Multi-stage Docker image

Build the binary in a toolchain stage and copy only the artifact into a small runtime stage.
Pin both tags; record the digests in the release notes.

```text
FROM rust:1.100.0-slim-trixie AS build
RUN apt-get update && apt-get install --yes --no-install-recommends musl-tools \
 && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /src
COPY . .
RUN cargo build --locked --release --target x86_64-unknown-linux-musl --bin app

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/app /app
ENTRYPOINT ["/app"]
```

Both tags existed on Docker Hub and gcr.io on 2026-11-12. The rules:

- A `static` or `scratch` runtime stage needs a static binary. Check it with `file` in the build
  stage. A glibc binary needs a runtime image with the same or a newer glibc than the build stage.
- `scratch` holds no files: no CA bundle, no `/etc/passwd`, no time zone data. Copy what the binary
  reads at run time.
- Run the final image once (`docker run --rm <image>`). A green build stage does not prove that
  the runtime stage holds everything the binary opens.

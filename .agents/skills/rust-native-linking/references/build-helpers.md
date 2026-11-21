# Native Build Helpers and Bindings

Read this reference when `build.rs` configures `cc`, `pkg-config`, `vcpkg`, or
`cmake`, forwards flags to a nested compiler, or generates bindings with
`bindgen` or `cbindgen`.

Contents: flag forwarding, `cc`, `pkg-config`, `vcpkg`, `cmake`, `bindgen`,
`cbindgen`, authoritative references.

## Forward flags to the correct compiler

Cargo removes `RUSTFLAGS` from the build-script environment. A nested `rustc` command must read
`CARGO_ENCODED_RUSTFLAGS`, whose arguments use the unit-separator character, and pass those Rust
flags deliberately. Do not split it on spaces.

Do not pass raw Rust flags to a C or C++ compiler yourself. Configure the selected native builder
with its target compiler and target-specific `CC_<target>` or `CFLAGS_<target>` inputs. Prefer
the builder crate's target-aware API. `cc` prints its resolved compiler command only when
`CC_ENABLE_DEBUG_OUTPUT=1` is set. `cargo build -vv` then shows it as a `running:` line. Keep that
line as build evidence without secrets. For `cmake`, call `Config::very_verbose(true)` to get
the verbose CMake output.

`cc` 1.2.2 and newer translates compatible codegen flags from `CARGO_ENCODED_RUSTFLAGS` into C
flags, because `Build::inherit_rustflags` defaults to `true`. For example,
`-Cforce-frame-pointers=yes` adds `-fno-omit-frame-pointer`, and `-Crelocation-model` and
`-Ccode-model` map to their C forms. A `RUSTFLAGS` or profile change can therefore change the C
objects. Call `.inherit_rustflags(false)` when the C flags must stay independent. When Rust
flags change, run `CC_ENABLE_DEBUG_OUTPUT=1 cargo build -vv` and diff the `running:` line.

## Use each native build helper for one job

### `cc`

Use `cc::Build` for a fixed list of source files. Let it select the compiler,
archiver, target flags, and C++ runtime. Do not invoke `gcc`, `clang`, `cl`, or
`ar` by name.

Track all source files and non-system headers. Respect target-qualified `CC`,
`CXX`, `AR`, `CFLAGS`, and `CXXFLAGS`. Keep custom flags behind
`is_flag_supported` or an explicit target condition. Use the `parallel`
feature only when native compilation is a measured bottleneck; Cargo already
coordinates build-script concurrency through its jobserver.

### `pkg-config`

Use `pkg_config::Config` when the target sysroot supplies a `.pc` file. Set a
minimum compatible version. Let the crate emit include paths, link search
paths, libraries, and transitive flags.

For cross-compilation, set target-qualified `PKG_CONFIG_PATH`,
`PKG_CONFIG_LIBDIR`, and `PKG_CONFIG_SYSROOT_DIR`. Do not set
`PKG_CONFIG_ALLOW_CROSS=1` without a target sysroot. A host `.pc` file can
produce a successful probe and an unusable target link.

### `vcpkg`

Use `vcpkg::Config` for a package in a vcpkg tree, primarily on Windows. Pin
the vcpkg baseline or manifest outside `build.rs`. Select the intended triplet
with `VCPKGRS_TRIPLET`. Treat `VCPKGRS_DYNAMIC=1` as a packaging change because
the final application must carry the selected DLLs. Match the triplet to the
target environment and CRT mode as [windows.md](windows.md) describes.

### `cmake`

Use `cmake::Config` when the upstream project already owns a CMake graph. Give
it source under `CARGO_MANIFEST_DIR` and let it install under `OUT_DIR`. Pass
only project options that affect the required library. Do not mirror CMake's
compiler, generator, or cross-target selection in ad hoc shell commands.

Inspect the returned install prefix. Emit the actual `lib`, `lib64`, or
configuration-specific directory. Do not assume one layout across platforms.

## Generate bindings in the correct direction

### C or C++ to Rust with `bindgen`

Prefer checked-in generated bindings when consumers must build without
`libclang`, or when the public native ABI changes only at release time. Run a
CI command that regenerates into a temporary file and fails on a diff.

Use build-time `bindgen` only when target macros or headers change the binding
shape and every build environment provides compatible `libclang`.

- Wrap only the public headers that define the ABI.
- Allowlist the required functions, types, and variables.
- Pass the target triple and the same sysroot and include paths as the native
  compiler.
- Install `bindgen::CargoCallbacks::new()` so header changes trigger
  regeneration. The deprecated `CargoCallbacks` constant does not track the
  top-level header.
- Write build-time output to `OUT_DIR` and include it with `include!`.
- Add a size, alignment, and call smoke test for the supported ABI. The call
  test also gives the `--warn-backrefs` lane an artifact that links the native
  archives.

Do not use generated declarations as soundness evidence. Review ownership,
nullability, aliasing, and callbacks with the `rust-unsafe` skill, and give each
`unsafe` call a `// SAFETY:` comment that names its invariant.

### Rust to C or C++ with `cbindgen`

Generate a public header from the Rust ABI in an explicit development or
release command. Check the header in when downstream build systems consume
source archives or published packages. Make CI regenerate and compare it.

Do not run `cbindgen` in `build.rs` only to update a checked-in header.
`build.rs` must not modify package source, and normal Rust consumers do not
need the header.

## Authoritative references

- [`cc` crate](https://docs.rs/cc/latest/cc/)
- [`pkg-config` crate](https://docs.rs/pkg-config/latest/pkg_config/)
- [`vcpkg` crate](https://docs.rs/vcpkg/latest/vcpkg/)
- [`cmake` crate](https://docs.rs/cmake/latest/cmake/)
- [bindgen user guide](https://rust-lang.github.io/rust-bindgen/)
- [cbindgen documentation](https://github.com/mozilla/cbindgen/blob/main/docs.md)

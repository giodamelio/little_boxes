---
description: Use when writing a build.rs or *-sys crate that builds, discovers, binds, links, or packages a native C or C++ library, or when diagnosing a native link or load failure on host or cross targets. Triggers on rustc-link-lib, pkg-config, vcpkg, CMake, bindgen, cbindgen, rpath, install name, windows-msvc, windows-gnu, import library, raw-dylib, PDB, LNK2019, ERROR_BAD_EXE_FORMAT, undefined reference, undefined symbol, library not loaded, or DLL not found. Not for wasm32 link errors; use rust-wasm.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-native-linking
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 992b12ab5c04e6bd577607c1e95216e41967e6a9
name: rust-native-linking
---
# Rust Native Linking

The `rust-unsafe` skill, when it is installed, owns FFI soundness; keep the
`improper_ctypes` and `improper_ctypes_definitions` lints on. General
cross-target setup, Android packaging, and `wasm32` link errors belong to the
`rust-cross-compilation`, `rust-android-build`, and `rust-wasm` skills.

Do not add a native build step when an existing Rust crate already owns the
same library. Reuse its `*-sys` crate and its `links` contract.

Read [references/windows.md](references/windows.md) when `TARGET` is a Windows
target. It covers the MSVC, GNU, and GNU LLVM toolchains, CRT selection,
import libraries and `raw-dylib`, vcpkg triplets, DLL loading, PDB files, and
final PE/COFF inspection.

## Completion evidence

`cargo check` does not link. A green check proves nothing about link inputs,
loader paths, or exported symbols. Prove each level that applies:

1. Build the exact target and crate type that ships.
2. Inspect the artifact architecture and native dependency table.
3. Inspect the required and exported symbols.
4. Run the packaged artifact outside `cargo run` and `cargo test`.
5. Repeat the check for each supported target policy or CI target. A link
   with `rust-lld` does not prove the archive order for a GNU ld target.

## Diagnose the first failing layer

Since Rust 1.97, the warn-by-default `linker_messages` lint prints linker
output as `warning: linker stderr: ...`; `-D warnings` does not deny it. Read
it before you change flags. Informational output goes to the allow-by-default
`linker_info` lint, which hid an ld64 `built for newer 'macOS' version` line
(Rust 1.98.1), the mark of `cc` objects built for another deployment target:
pass `cargo rustc --bin <app> -- -W linker-info` while you
diagnose. To fail CI on linker output, build with `CARGO_BUILD_WARNINGS=deny`
(the `rust-lints` skill owns the gate level).

| Symptom | Likely layer | First evidence | Fix |
|---|---|---|---|
| `cannot find -lfoo`, `unable to find library -lfoo`, `library 'foo' not found`, or `LNK1181` | Link search | Exact linker command and artifact directory | Correct discovery or `rustc-link-search`; do not copy to a global directory |
| `undefined reference`, `undefined symbol`, or `LNK2019` | Symbol or order | Undefined name plus provider symbol table | Add the real provider, correct mangling, or order consumer before provider |
| Links with `rust-lld` on x86_64 Linux, `undefined reference` with GNU ld or on another Linux target | Archive order that `rust-lld` tolerates | `--warn-backrefs` lane output | Emit each consumer before its provider |
| `undefined reference to 'open64'`, `fstat64`, or another `*64` name on `*-linux-musl`, Rust 1.93+ | Bundled musl 1.2.5 has no legacy LFS64 symbols | `cargo tree -i libc` and the object that references the name | Update `libc` to 0.2.146 or newer; rebuild the C input with the standard names. `-D_LARGEFILE64_SOURCE` is only a short-term shim |
| Duplicate symbol or `LNK2005` | Ownership | Link map and all `links` owners | Remove the second provider or archive copy |
| Wrong ELF class, bad CPU type, or `0xc000007b` | Architecture | Artifact header and `TARGET` | Rebuild every native input for the target architecture |
| `GLIBCXX_* not found` | C++ runtime version | `DT_NEEDED`, symbol versions, packaged runtime | Use one compatible C++ runtime policy and package it when required |
| `library not loaded` on macOS | Install name or run path | `otool -L` and `LC_RPATH` | Correct `@rpath`, `@loader_path`, embedding, and signing |
| DLL not found on Windows | Packaging or transitive DLL | `dumpbin /DEPENDENTS` recursively | Ship the correct DLLs in an intended search location |
| Works in `cargo run`, fails from package | Loader environment | Run outside Cargo and inspect dependency table | Add package-relative run path or package the DLL |
| Native build uses host headers while crossing | Host or target mix | `HOST`, `TARGET`, compiler command, sysroot, `cfg!(target_*)` in `build.rs` | Read `TARGET` and `CARGO_CFG_TARGET_*`, not `cfg!`; select target-qualified tools, headers, libraries, and probes |
| Rebuilds on every edit | Change detection | `cargo build -vv` build-script reason | Add precise `rerun-if-changed` and `rerun-if-env-changed` rules |
| Header changed but bindings did not | Generation inputs | Regeneration diff and build-script output | Track included headers or make checked-in generation a CI gate |

Add no linker flag until you can name the missing file, symbol, or path.

## Inspect final artifacts

Inspect the exact shipped file with `readelf`, `nm`, `otool`, `lipo`, or
`dumpbin`. Recurse through every dynamic dependency. Read
[references/artifact-inspection.md](references/artifact-inspection.md) when you
inspect an artifact: it has the commands, the v0 symbol mangling rule (since
Rust 1.97 a `_ZN` grep finds nothing), and why `-Wl,--version-script` does not
hide `cdylib` exports.

## Emit linker instructions in dependency order

- Prefer structured `cargo::rustc-link-lib` and `cargo::rustc-link-search`
  instructions over raw linker arguments.
- Emit a consumer archive before the libraries that satisfy its undefined
  symbols. Fix the order before adding `--start-group` or `+whole-archive`;
  both can hide a dependency cycle.
- `rust-lld`, the default for `x86_64-unknown-linux-gnu` since Rust 1.90,
  tolerates a backward archive reference that GNU ld rejects. Catch it with a
  `-Wl,--warn-backrefs -D linker-messages` CI lane.
- Do not duplicate instructions that `cc`, `pkg-config`, or `vcpkg` already
  emits.
- A `cargo::rustc-link-arg-*` instruction affects only the package that emits
  it: a `foo-sys` script cannot add a run path to a dependent `cdylib`.

Read [references/link-order.md](references/link-order.md) when you write link
instructions, fix an order defect, or set up the `--warn-backrefs` lane.

## Select one integration path

| Input | Use | Do not use |
|---|---|---|
| A maintained crate already links the library | Its `*-sys` crate | A second `links` owner |
| A few C, C++, assembly, or CUDA source files | `cc` | Hand-written compiler and archiver commands |
| A Unix system package with a `.pc` file | `pkg-config` | Hard-coded `/usr/lib` or `/usr/local/lib` |
| A Windows package in a vcpkg tree | `vcpkg` | A machine-specific absolute `.lib` path |
| An upstream CMake project | `cmake` | Reimplementing its target graph in `cc` |
| C or C++ headers must become Rust declarations | `bindgen` | Hand-copied declarations that drift |
| A Rust ABI must expose a C or C++ header | `cbindgen` | Running `bindgen` in the wrong direction |

Use the upstream build system when it carries probes or platform rules. Read
[references/build-helpers.md](references/build-helpers.md) when `build.rs`
configures one of these helpers or generates bindings. It holds each helper's
tracking, cross-compilation, and CI regeneration rules.

Choose system or bundled source explicitly. Do not silently fall back from a
system library to bundled source. The fallback changes patch ownership,
licensing, ABI, and static or dynamic behavior. If both modes are required,
put the choice behind one documented feature in the single `*-sys` crate.

## Put native ownership in one `*-sys` crate

A small `foo-sys` package declares `links = "foo"`, builds or discovers the
library, emits the link instructions once, and holds the raw declarations.
Keep the safe API in a separate crate. Cargo permits only one package for each
`links` value in a graph, which prevents duplicate symbols. Do not make system
and bundled providers separate packages with the same `links` value. `DEP_*`
metadata reaches only immediate dependents.

## Keep `build.rs` deterministic

Treat `build.rs` as a host executable that produces target artifacts.

- Read inputs under `CARGO_MANIFEST_DIR`.
- Write generated files, object files, and native build trees only under
  `OUT_DIR`.
- Do not modify `src/`, a registry checkout, or a vendored source tree.
- Do not assume that `OUT_DIR` is empty. Replace outputs atomically or let the
  selected native builder manage its own subdirectory.
- Do not download source, install packages, or inspect unrelated host state.
- Return a non-zero status for a required native step that fails.
- Print diagnostics to stderr. Reserve stdout for `cargo::` instructions.
- Run `cargo build -vv` to see the command stream and saved script output.

Emit one `rerun-if-changed` or `rerun-if-env-changed` rule for every direct
file and external environment variable that the script reads, never for a
variable Cargo sets. Read [references/sys-crate.md](references/sys-crate.md)
when you write a `foo-sys` package, its `cargo::metadata`, or the change rules;
it has a complete `build.rs`.

Pass flags to each compiler through its own channel: a nested `rustc` reads
`CARGO_ENCODED_RUSTFLAGS` (never split it on spaces), and `cc` 1.2.2
or newer copies compatible Rust codegen flags into C flags. Read
[references/build-helpers.md](references/build-helpers.md) when `build.rs` runs
a nested compiler or when a Rust flag change must not change the C objects.

## Separate host and target

A build script compiles and runs on `HOST`. It produces native code for
`TARGET`.

- Read `TARGET` and `CARGO_CFG_TARGET_*` for target decisions.
- Do not use `cfg!(target_os)` in `build.rs`; it describes the host.
- Use `HOST != TARGET` as the cross-compilation test.
- Run generators and build tools for `HOST`.
- Compile libraries, probe headers, and select ABI files for `TARGET`.
- Never execute a target probe program from `build.rs`; a cross target binary
  cannot run on the host. Use compile-only feature checks or target metadata.
- Pass the resolved target linker or toolchain file to the native builder.
- Keep host tools out of target link search paths.

A host success is not cross-target evidence.

## Choose static, dynamic, or framework linking

| Mode | Select when | Required proof |
|---|---|---|
| Static archive | One artifact and license policy permit it | Symbols are present once; CRT and C++ runtimes match |
| Dynamic library | The platform or update policy owns a shared library | Loader path and every transitive library work after packaging |
| Apple framework | The dependency ships as a framework | Framework search path, architecture slices, embedding, signing |

Use `rustc-link-lib=static=foo`, `dylib=foo`, or `framework=Foo`. Do not infer
the mode from the first file in a search directory. On
Windows, a `.lib` can be an import library; `references/windows.md` covers the
difference.

## Make runtime loading a packaging property

`cargo::rustc-link-search` solves link-time discovery only. A `cargo run` or
`cargo test` smoke test can pass while the packaged program fails. Run the
installed artifact outside Cargo. The `rust-crate-release` skill owns the
package and publish gate.

Do not run `ldd` on an untrusted executable: it can run the program. Inspect
`DT_NEEDED`, `RUNPATH`, and symbols with `readelf` first.

Read [references/runtime-loading.md](references/runtime-loading.md) when you set
an ELF `RUNPATH` or a Mach-O install name and `LC_RPATH`, embed frameworks in a
bundle, or see `ld: library 'foo' not found` on macOS.

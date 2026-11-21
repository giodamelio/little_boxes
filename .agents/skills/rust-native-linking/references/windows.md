# Windows Native Linking

Read this reference when `TARGET` is a Windows target. Keep application
installation, signing, and update policy in the application packaging
workflow. This reference owns the native ABI, link inputs, DLL payload, and
PE/COFF evidence.

Contents: target contract, CRT policy, vcpkg triplets, archives and import
libraries, exported names, DLL loading, debug symbols, final PE/COFF checks,
and a failure triage table.

## Freeze the target contract

Record the full Rust target before you select a compiler or library. Do not
route on `CARGO_CFG_TARGET_OS=windows` alone.

| Cargo input | Decision |
|---|---|
| `TARGET` | Select the exact Rust target and native toolchain |
| `CARGO_CFG_TARGET_ENV` | Select `msvc` or GNU-family artifacts; use `CARGO_CFG_TARGET_ABI` to split `gnu` from `gnullvm` |
| `CARGO_CFG_TARGET_ABI` | `llvm` marks `-pc-windows-gnullvm`; empty marks `-pc-windows-gnu` and `-pc-windows-msvc` |
| `CARGO_CFG_TARGET_ARCH` | Select `x86`, `x86_64`, `aarch64`, or another supported machine |
| `CARGO_CFG_TARGET_FEATURE` | Detect the `crt-static` policy |
| `HOST` | Select generators and other tools that must run during the build |

Use `CARGO_CFG_*` values in `build.rs`. The script runs for `HOST`, so
`cfg!(target_env)` and `cfg!(target_arch)` describe the wrong machine during a
cross build.

Choose one target environment for the complete native link:

| Rust target suffix | Native contract | Typical library files |
|---|---|---|
| `-pc-windows-msvc` | Microsoft ABI and a `link.exe`-like linker | Static or import `.lib`, `.dll` |
| `-pc-windows-gnu` | MinGW-w64 with GCC and Binutils; MSVCRT by default | Static `.a`, import `.dll.a`, `.dll` |
| `-pc-windows-gnullvm` | MinGW-w64 with LLVM tools (llvm-mingw or an MSYS2 `CLANG*` environment); UCRT | Static `.a`, import `.dll.a`, `.dll` |

Do not mix MSVC and GNU object files or C++ libraries. Both produce PE files,
but their compiler ABI, symbol decoration, runtime, and archive conventions
can differ. Do not reuse `-gnu` archives for a `-gnullvm` target either: the
C runtime (MSVCRT or UCRT), the C++ runtime (libstdc++ or libc++), and the
unwinder (libgcc or libunwind) can all differ. Build every native input with
the toolchain selected by `TARGET`.

Rust 1.100 makes `i686-pc-windows-gnu` Tier 2 without host tools. Cross-compile
32-bit outputs from a supported host; do not require a native 32-bit toolchain.
On `windows-gnullvm`, Rust 1.100 links libunwind statically. Recheck the final
DLL import list instead of packaging an old unwinder DLL by habit.

For `-pc-windows-gnu`, Rust 1.98 documents GNU Binutils 2.44, GCC 14.2, and
mingw-w64 12.0.0 as the oldest supported tools. Older tools, especially
Binutils, may not work. Record the versions in build evidence.

Every Windows Rust target uses Windows calling conventions for `extern "C"`.
This does not make a C++ ABI portable. Put an `extern "C"` shim around a C++
interface. Keep allocation, exceptions, standard library types, and ownership
inside the toolchain boundary.

Use a matching native architecture. Map `x86` to Win32 or x86, `x86_64` to
x64, and `aarch64` to ARM64. Do not infer the target architecture from the
host process or the directory name of an SDK.

## Select one CRT policy

For MSVC targets, make Rust and every native object agree on the CRT mode:

| Rust target feature | MSVC release option | Runtime |
|---|---|---|
| `+crt-static` | `/MT` | Static CRT |
| `-crt-static` | `/MD` | DLL CRT |

Use `/MTd` or `/MDd` only for local debug artifacts. Microsoft does not permit
redistribution of the debug CRT. Do not ship a native debug library in a
release Rust artifact.

Set the Rust policy with `-C target-feature=+crt-static` or
`-C target-feature=-crt-static`. Read the comma-separated
`CARGO_CFG_TARGET_FEATURE` value in `build.rs`, then give the equivalent mode
to the selected C or C++ build helper. Inspect the final binary because some
targets ignore unsupported CRT feature changes.

Do not pass memory, file handles, locales, environment objects, C++ standard
library objects, or exceptions across a DLL boundary unless the ABI defines
the ownership contract. Prefer opaque handles and matching allocate and free
functions in the same DLL. Matching `/MD` versions reduces CRT conflicts, but
it does not replace an ownership contract.

## Match vcpkg triplets

Pin the vcpkg baseline or manifest outside `build.rs`. Select the triplet from
the target environment, architecture, native library mode, and CRT mode. Do
not accept the host default.

Common MSVC examples are:

| Requested native policy | Example x64 triplet |
|---|---|
| DLL library and DLL CRT | `x64-windows` |
| Static library and static CRT | `x64-windows-static` |
| Static library and DLL CRT | `x64-windows-static-md` (community triplet) |

Use the corresponding `x86-` or `arm64-` triplet for another architecture.
Set `VCPKGRS_TRIPLET` for the Rust `vcpkg` helper. If an upstream CMake build
uses vcpkg directly, set `VCPKG_TARGET_TRIPLET`. Set a separate host triplet
only for vcpkg tools that execute during the build.

Treat a triplet change as an ABI and packaging change. A triplet can control
the CPU, compiler toolset, library linkage, and CRT linkage. Confirm the
actual output because a port can constrain supported linkage combinations.

Do not use an MSVC vcpkg triplet for a `windows-gnu` target. Use native
MinGW-w64 packages built for the exact GNU target and architecture.

## Distinguish archives and import libraries

A `.lib` extension does not prove static linkage:

- A static library stores code that enters the final image.
- A DLL import library stores link-time records for exports in a DLL.
- The matching DLL remains a runtime dependency.

For MSVC artifacts, inspect libraries with `dumpbin /LINKERMEMBER` and
`dumpbin /SYMBOLS`. Import records commonly contain `__imp_` symbols and an
import descriptor. Inspect the final image import table before you decide
which DLLs to package.

For GNU artifacts, distinguish static `.a` files from `.dll.a` import
libraries. Use the MinGW-w64 `objdump` and `nm` from the same target toolchain.
Do not rename one format to imitate another.

When no import library exists, or when a cross build cannot produce one, use
`#[link(name = "foo", kind = "raw-dylib")]` on the `extern` block. rustc then
generates the import library at link time. On `-pc-windows-gnu`, rustc runs
Binutils `dlltool` for this step. Put the target `dlltool` on `PATH` or pass
`-C dlltool=<path>`. MSVC and `-gnullvm` targets need no external tool. Other
targets reject the kind with `error[E0455]`, so guard it with
`cfg_attr(windows, ...)` in portable code.
`foo.dll` is still a runtime dependency.

When LINK builds a DLL with exports, it normally creates the MSVC import
library. Preserve the DLL and import library as one versioned output set. Use
the import library only at link time. Put the DLL and its transitive DLL
dependencies in the application payload.

## Keep exported names stable

Prefer a narrow C ABI for a DLL consumed outside its compiler toolchain.
Declare exported C++ functions with `extern "C"` when the API does not require
C++ overloads or classes. Match the Rust `extern` ABI and the native calling
convention, especially on 32-bit x86.

Do not guess a decorated name from source. Name decoration changes with the
language, calling convention, parameter types, architecture, compiler, and
toolset. Inspect it with `dumpbin /EXPORTS`, `dumpbin /SYMBOLS`, or the GNU
toolchain equivalent. Use `undname` only to explain an MSVC C++ name.

Use a module-definition file or the native export attribute when the public
DLL contract requires a fixed export name. Do not publish compiler-specific
C++ decorated names as a stable ABI unless every consumer uses the same
supported toolchain contract.

## Keep DLL loading out of the developer environment

An import library does not place a DLL at runtime. Cargo can extend `PATH`
with target-directory link search paths for `cargo run` and `cargo test`.
That environment is not package evidence.

Put private DLLs in an application-owned location selected by the packaging
policy. Test the installed executable outside Cargo with a clean developer
`PATH`. Inspect every transitive import because Windows searches dependencies
by module name even when the first DLL was loaded with a full path.

For explicit loading, use an absolute path with `LoadLibraryExW` and
`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS`. An
absolute path alone anchors only the first DLL; Windows still resolves its
transitive dependencies by module name. A process-wide policy can instead use
`SetDefaultDllDirectories` and scoped `AddDllDirectory` entries. Do not search
the current directory. Test loading from an attacker-controlled current
directory. Do not add a global `PATH` entry as the product contract.

Use delay-load only when an existing product policy makes a DLL optional or
defers its startup cost. Add `/DELAYLOAD:<name>.dll` and `delayimp.lib` to the
link of the final-artifact crate, not its `*-sys` dependency. Do not distribute
`delayimp.lib` as a runtime file. The default helper uses `LoadLibrary` and
raises structured exceptions for a missing DLL or export; an ordinary Rust
`Result` at the call site does not contain that failure. Before a delayed call,
either preload through the safe `LoadLibraryExW` policy and probe the required
exports, or provide the documented delay-load failure hook or custom helper
that produces a controlled fallback. Confirm the delay import with `dumpbin
/IMPORTS`. Do not add delay-load only to hide a packaging defect.

## Preserve debug symbols

Decide whether release diagnostics require symbols. For MSVC targets, keep
the final Rust PDB and the PDB files for native code under the same retention
policy as the matching binary. Do not ship private PDB files beside the
application unless the product policy requires it. Store them by build
identity in the approved symbol archive.

For MSVC native code, `/Z7` keeps compiler debug information in object files.
`/Zi` writes compiler information to a PDB that must be available when LINK
consumes the object or library. `/DEBUG:FULL` creates a final PDB that can be
used without the original objects. Do not use `/DEBUG:FASTLINK`: its PDB
depends on the original objects, and Visual Studio 2026 removed the option.

Use `dumpbin /PDBPATH:VERBOSE <artifact>` to inspect the recorded PDB path.
Verify that the archived PDB matches the shipped EXE or DLL. A file with the
same base name from another build is not valid evidence.

## Verify the final PE/COFF set

Run these commands in an MSVC Developer Command Prompt on the exact files that
ship:

```text
dumpbin /HEADERS <app.exe>
dumpbin /HEADERS <plugin.dll>
dumpbin /DEPENDENTS <app.exe>
dumpbin /IMPORTS <app.exe>
dumpbin /EXPORTS <plugin.dll>
dumpbin /PDBPATH:VERBOSE <app.exe>
dumpbin /HEADERS <native.lib>
dumpbin /LINKERMEMBER <native.lib>
dumpbin /SYMBOLS <native.lib>
```

For a GNU target, use tools from the same MinGW-w64 target:

```text
<target>-objdump -f <app.exe>
<target>-objdump -p <app.exe>
<target>-nm -g <native-library>
```

Prove these properties:

- Every EXE, DLL, object, static library member, and import library uses the
  intended machine architecture.
- Every unresolved import has one intended provider.
- Exported names and calling conventions match the binding declarations.
- The import table contains only intended DLL dependencies.
- Every non-system DLL and transitive DLL exists in the packaged layout.
- The selected Rust, native, vcpkg, and CRT policies agree.
- The packaged executable starts and exercises one native call without Cargo,
  Visual Studio, vcpkg, or a developer toolchain on `PATH`.
- The retained PDB files match the shipped build when symbol retention is in
  scope.

## Triage Windows failures

| Symptom | First evidence | Fix |
|---|---|---|
| `LNK1112` or `0xc000007b` | `/HEADERS` machine type for every input and DLL | Rebuild the wrong-architecture input for `TARGET` |
| `ERROR_BAD_EXE_FORMAT` (`193` or `0xC1`) | `/HEADERS` or target `objdump -f` for the EXE and every DLL | Rebuild the non-PE, wrong-environment, or wrong-architecture input for the exact `TARGET` |
| `LNK2019` | Exact decorated name and provider symbols | Correct the ABI, calling convention, import library, or export |
| `LNK2019` for a registry or security API, such as `RegOpenKeyExW` | Which system library exports the symbol | Since Rust 1.87, `std` no longer links `advapi32`; emit `cargo::rustc-link-lib=advapi32` from the `*-sys` build script |
| `LNK2038` with `RuntimeLibrary` | `/MD` or `/MT` policy for every native object | Rebuild all inputs with one compatible CRT mode |
| Link succeeds but DLL is absent | `/IMPORTS` and `/DEPENDENTS` | Package the matching DLL and recurse through its dependencies |
| Works in `cargo run` only | Clean `PATH` launch outside Cargo | Remove dependence on Cargo's loader environment |
| DLL loads but entry point is absent | Import name and `/EXPORTS` | Use the matching DLL version or correct the exported name |
| Symbols do not resolve in a debugger | `/PDBPATH:VERBOSE` and symbol archive identity | Retain and index the PDB from the same build |

## Authoritative references

- [Rust Windows MSVC targets](https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html)
- [Rust Windows GNU targets](https://doc.rust-lang.org/rustc/platform-support/windows-gnu.html)
- [Rust Windows GNU LLVM targets](https://doc.rust-lang.org/rustc/platform-support/windows-gnullvm.html)
- [Rust `dylib` versus `raw-dylib`](https://doc.rust-lang.org/reference/items/external-blocks.html#dylib-versus-raw-dylib)
- [Rust static and dynamic C runtimes](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)
- [Cargo build-script environment](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts)
- [Microsoft CRT library features](https://learn.microsoft.com/cpp/c-runtime-library/crt-library-features)
- [Microsoft `/MD` and `/MT`](https://learn.microsoft.com/cpp/build/reference/md-mt-ld-use-run-time-library)
- [Microsoft import libraries](https://learn.microsoft.com/cpp/build/reference/working-with-import-libraries-and-export-files)
- [Microsoft decorated names](https://learn.microsoft.com/cpp/build/reference/decorated-names)
- [Microsoft vcpkg triplets](https://learn.microsoft.com/vcpkg/concepts/triplets)
- [Microsoft vcpkg Windows triplets](https://learn.microsoft.com/vcpkg/users/platforms/windows)
- [Microsoft DLL search order](https://learn.microsoft.com/windows/win32/dlls/dynamic-link-library-search-order)
- [Microsoft delay-loaded DLLs](https://learn.microsoft.com/cpp/build/reference/linker-support-for-delay-loaded-dlls)
- [Microsoft PDB linker input](https://learn.microsoft.com/cpp/build/reference/dot-pdb-files-as-linker-input)
- [Microsoft `/DEBUG`](https://learn.microsoft.com/cpp/build/reference/debug-generate-debug-info)
- [Microsoft PE/COFF format](https://learn.microsoft.com/windows/win32/debug/pe-format)
- [Microsoft DUMPBIN options](https://learn.microsoft.com/cpp/build/reference/dumpbin-options)

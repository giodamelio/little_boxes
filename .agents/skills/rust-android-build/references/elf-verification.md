# Native Artifact Verification and Size Gate

Read this file when you write or change the release gate script for the Rust `.so` files, or when
the size gate fails. It says what to inspect in each built library, in what order, how to audit a
size regression, and how to wire the checks into CI.

Contents: check order; file and ELF header; LOAD alignment and page-size assumptions in code;
`DT_NEEDED` and prebuilt `GNU_RELRO`; exported symbols, the allowed set, and the allowlist diff;
build ID; size gate and size audit; CI wiring; triage.

All commands use the NDK LLVM tools. Resolve them once (the macOS NDK uses `darwin-x86_64` on
Apple silicon too):

```bash
NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$(uname | tr '[:upper:]' '[:lower:]')-x86_64/bin"
```

Substitute your own library name for `libnative.so`.

## Check order

Run the checks in this order. Each one is cheaper than the next, and an early failure makes the
later results meaningless.

| Order | Check | Tool | Pass condition |
|-------|-------|------|----------------|
| 1 | The file exists for every shipped ABI | `test -f` | One `.so` per ABI directory |
| 2 | The file is an ELF shared object for the right machine | `llvm-readelf -h` | `Type: DYN`, machine matches the ABI |
| 3 | LOAD segment alignment | `llvm-readelf -lW` | `0x4000` or larger on 64-bit ABIs; 32-bit per project policy |
| 4 | Shared-library dependencies | `llvm-readelf -d` | Only system libraries or libraries shipped in the same ABI directory |
| 5 | Exported dynamic symbols | `llvm-readelf --dyn-syms` | Exactly the boundary allowlist |
| 6 | Build ID present | `llvm-readelf -n` | A GNU build-ID note exists |
| 7 | Size against baseline | `stat` plus the baseline file | Inside the budget |

Point the checks at the merged native-library tree that the packaging step consumes. A check
against one hand-picked build directory can pass while the packaged tree still holds a stale
artifact. This gate does not open an APK or AAB. The package and device checks in
[SKILL.md](../SKILL.md) cover the final archive.

## 1-2. File presence and ELF header

```bash
"$NDK_BIN/llvm-readelf" -h <lib-dir>/arm64-v8a/libnative.so | grep -E 'Type|Machine'
# Type:    DYN (Shared object file)
# Machine: AArch64
```

Expected machine per ABI:

| ABI | `Machine` |
|-----|-----------|
| `arm64-v8a` | `AArch64` |
| `armeabi-v7a` | `ARM` |
| `x86_64` | `Advanced Micro Devices X86-64` |
| `x86` | `Intel 80386` |

A machine mismatch means that the copy step put the output of one Rust target into the directory
of another ABI. Check the triple-to-ABI mapping in the Gradle task.

## 3. 16 KiB LOAD segment alignment

```bash
"$NDK_BIN/llvm-readelf" -lW <lib-dir>/arm64-v8a/libnative.so \
  | awk '/LOAD/ {print $NF}' \
  | sort -u
# Expected on arm64-v8a and x86_64: 0x4000, or a larger power of two such as 0x10000
```

The last column of a `readelf -lW` program-header line is the alignment. Run the command once per
file. The final link gives every LOAD segment of one file the same alignment, so `sort -u` prints
one value. Two values mean that the command read more than one file.

On `armeabi-v7a` and `x86`, the NDK default is `0x1000`. Require `0x4000` there only when the
project policy applies the 16 KiB flag to 32-bit ABIs.

Aligned segments do not fix code that assumes a 4096-byte page:

- An `mmap` or `mprotect` size that assumes a 4096-byte page is not 16 KiB aligned. The kernel
  rounds the mapping up, and the code keeps using its smaller size. Read the page size at run
  time: `sysconf(_SC_PAGESIZE)` in C, `libc::sysconf(libc::_SC_PAGESIZE)` in Rust.
- For an undefined `PAGE_SIZE` in C code on a 64-bit ABI, replace the macro with
  `sysconf(_SC_PAGESIZE)`. Do not define `__BIONIC_DEPRECATED_PAGE_SIZE_MACRO`, because it
  restores `4096` and hides the bug.

## 4. Shared-library dependencies

```bash
"$NDK_BIN/llvm-readelf" -d <lib-dir>/arm64-v8a/libnative.so | grep NEEDED
```

A Rust `cdylib` normally needs only system libraries such as `libc.so` and `libdl.so`. Any other
entry is a prebuilt shared library. It must ship in the same ABI directory, or the load fails on a
device and not on your machine. It also keeps its own LOAD alignment: run check 3 on it. A static
C archive linked into the Rust `cdylib` is not listed here, and it takes the alignment of the
final link.

Rebuild a misaligned prebuilt with a linker option in its build system, for example
`LDFLAGS=-Wl,-z,max-page-size=16384` or CMake
`target_link_options(<target> PRIVATE "-Wl,-z,max-page-size=16384")`. A linker option in `CFLAGS`
does not reach the final link.

Also check the `GNU_RELRO` segment of each prebuilt `.so` on a 64-bit ABI. The Android 16 KB
page-size guide (developer.android.com/guide/practices/page-sizes, read 2026-10-31) says that a
RELRO end (`VirtAddr` plus `MemSiz`) that is not 16 KB aligned crashes the app with a
segmentation fault. The rule applies when `GNU_RELRO` shares a `LOAD` segment with writable data
such as `.data` or `.bss`. A RELRO that fills its own `LOAD` passes. A link with NDK r27 or older
can produce the bad layout. Rebuild it with both `-Wl,-z,max-page-size=16384` and
`-Wl,-z,common-page-size=16384`, or with NDK r28 or later. The LOAD check above does not catch
this layout. Run this check once per prebuilt file:

```bash
so=<lib-dir>/arm64-v8a/libprebuilt.so
read -r rstart rsize < <("$NDK_BIN/llvm-readelf" -lW "$so" | awk '$1 == "GNU_RELRO" {print $3, $6}') || true
if [ -n "${rstart:-}" ]; then
  rend=$(( rstart + rsize ))
  while read -r lstart lsize; do
    lend=$(( lstart + lsize ))
    if (( lstart <= rstart && rend < lend && rend % 0x4000 != 0 )); then
      printf 'FAIL %s: GNU_RELRO ends at 0x%x inside a LOAD that ends at 0x%x\n' "$so" "$rend" "$lend"
      exit 1
    fi
  done < <("$NDK_BIN/llvm-readelf" -lW "$so" | awk '$1 == "LOAD" {print $3, $6}')
fi
```

Columns 3 and 6 of a program-header line are `VirtAddr` and `MemSiz`. A file with no `GNU_RELRO`
line passes.

## 5. Exported symbol allowlist

rustc owns the export list of a `cdylib`. It passes its own `-Wl,--version-script` and
`-Wl,--no-undefined-version`, and it exports every `#[unsafe(no_mangle)]` and
`#[unsafe(export_name)]` item in the crate graph, even one in a dependency that the library never
calls, under fat LTO too. A checked-in `--version-script` does not narrow that list: with Rust
1.98.1 and NDK r30 `ld.lld`, `global: Java_*; local: *;` left a `#[unsafe(no_mangle)]` helper
exported, and a map that names an absent symbol, such as `JNI_OnUnload`, breaks the link. Do not
add a version script. Remove a leak at its source: delete the attribute, or turn off the
dependency feature that adds it.

The allowed set depends on the boundary:

- JNI name-based binding: `JNI_OnLoad` and `JNI_OnUnload` when the crate defines them, and the
  `Java_*` methods.
- `RegisterNatives`: `JNI_OnLoad`, and `JNI_OnUnload` when defined. Registered methods need no
  exported name.
- UniFFI: the `external fun` functions that the generated Kotlin file declares in `UniffiLib` and
  `IntegrityCheckingUniffiLib` (bound with JNA `Native.register`), plus the `UNIFFI_META_*` data
  symbols that the UniFFI macros export, plus `JNI_OnLoad` when the crate defines one to seed
  `JavaVM::singleton()` for callback threads. Kotlin generation writes no C header (as of
  uniffi-bindgen 0.32.2). A `<namespace>FFI.h` header exists only when the build also generates
  Swift. The generated code does not declare the `UNIFFI_META_*` symbols, and uniffi-bindgen
  reads them from the unstripped library, so do not remove them.

Derive `expected-exports.txt` from the JNI registration table, or for UniFFI from the
`external fun` names in those two Kotlin objects. Add the `UNIFFI_META_*` names from the dynamic
symbols of the same build, and `JNI_OnLoad` when the crate defines it. Sort the file, and compare it with the defined dynamic symbols of each final
`.so`:

```bash
"$NDK_BIN/llvm-readelf" --dyn-syms --wide <lib-dir>/arm64-v8a/libnative.so \
  | awk '$5 ~ /^(GLOBAL|WEAK)$/ && $7 != "UND" && $4 ~ /^(FUNC|OBJECT)$/ {print $8}' \
  | sort -u > actual-exports.txt
comm -23 actual-exports.txt expected-exports.txt > unexpected-exports.txt
comm -13 actual-exports.txt expected-exports.txt > missing-exports.txt
test ! -s unexpected-exports.txt && test ! -s missing-exports.txt
```

The filter keeps data symbols (`OBJECT`, for example a `#[unsafe(no_mangle)] pub static`) and drops
imports (`UND`) such as `__cxa_finalize`. An unexpected symbol is an ABI leak: any process that
can `dlopen` the library can call it, and it pins the signature. A missing name-based export fails
at run time with `UnsatisfiedLinkError`.

Do not filter `llvm-objdump -T` output on ` DF `: that keeps functions only, so an exported
`#[unsafe(no_mangle)] pub static` passes.

## 6. Build ID

```bash
"$NDK_BIN/llvm-readelf" -n <lib-dir>/arm64-v8a/libnative.so | sed -n 's/.*Build ID: //p'
```

The command must print one hex ID. The `-Wl,--build-id=sha1` flag in [SKILL.md](../SKILL.md)
produces it; that file says why a plain `--build-id` fails.

## 7. Size gate

Keep a checked-in baseline file that maps library name and ABI to a byte count. The default
thresholds are at most 128 KiB growth for one tracked library, and total growth across all
tracked libraries of at most the tighter of 2% or 256 KiB; tune them per product. The per-library
limit catches
one crate that ballooned, and the total limit catches many small increases that each pass the
per-library limit.

Read byte counts from the baseline file, not from documentation. Update the baseline in a separate
commit that states the reason. A baseline update inside a feature commit hides the growth from
review.

### Audit a regression

`cargo bloat` builds with `cargo build` and accepts only bin, dylib, and cdylib targets. On a
`crate-type = ["lib"]` package it stops with `only 'bin', 'dylib' and 'cdylib' crate types are
supported`. Rank the symbols of the unstripped `.so` from the Gradle task instead:

```bash
"$NDK_BIN/llvm-nm" -S --size-sort --radix=d -C <unstripped>/libnative.so | tail -30
```

Or run `cargo bloat` on a thin shim package that declares `crate-type = ["cdylib"]` and holds
`pub use <ffi_crate>;` in its `lib.rs`. Without that line, rustc does not link the FFI crate, and
`cargo bloat` measures an empty library.

```bash
cargo bloat --locked --profile android-jni --target aarch64-linux-android -p <cdylib-shim> --crates -n 30
cargo bloat --locked --profile android-jni --target aarch64-linux-android -p <cdylib-shim> -n 30   # by function
```

Common causes: a generic that monomorphizes into many copies (move the body into a non-generic
inner function), a new transitive dependency (diff `cargo tree --locked -p <ffi-crate>`), or a
build that selected a profile without fat LTO.

Of the two ICF modes, `-Wl,--icf=safe` folds little Rust code: it needs `.llvm_addrsig` tables,
and rustc 1.99.1 objects for `aarch64-linux-android` carry none. [SKILL.md](../SKILL.md) has the
rule for `-Wl,--icf=all`.

## Wiring the gate into CI

1. Run the checks against the merged native-library tree, after the merge task and before
   packaging.
2. Fail the job on any check; do not warn. A warning in a native build is ignored until a store
   review rejects the release.
3. Print the file, the ABI, and the measured value on failure. A gate that prints only "failed"
   costs an extra debugging round trip.
4. Run the complete declared shipping ABI matrix on the release path. A pull-request job may check
   one ABI to save runner time.
5. Keep the gate script in the repository, not in the CI configuration, so that you run the same
   logic locally.

## Triage table

| Symptom | Cause | Action |
|---------|-------|--------|
| `sort -u` prints `0x1000` and `0x4000` | The command read several files | Run it per file; the `0x1000` file is a separately linked `.so` or a 32-bit build |
| `Machine` is wrong for the directory | The copy step mapped a triple to the wrong ABI | Fix the triple-to-ABI mapping |
| Size grew on one ABI only | A target-specific code path or an intrinsic fallback | `llvm-nm --size-sort` on that target's unstripped `.so`, or `cargo bloat` on a `cdylib` package |

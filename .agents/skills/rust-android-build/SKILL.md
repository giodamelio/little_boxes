---
description: Use when building, verifying, or packaging a Rust cdylib for Android - NDK linker setup and per-ABI rustflags in .cargo/config.toml, 16 KiB page alignment and the Play 16 KB page size check, exported ELF symbols, .so size budgets, a Gradle task that fills jniLibs, native debug symbols, installed release smoke tests, or a reusable AAR or Prefab package. Not for JNI function code; use rust-jni.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-android-build
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 834237a3eb7e973733752ccda95ad60309593b38
name: rust-android-build
---
# Rust Android Build

As of 2026-09, the current NDK LTS is r30 (`30.0.16248370`,
[NDK downloads](https://developer.android.com/ndk/downloads)), and Rust supports the most recent
LTS ([rustc Android platform page](https://doc.rust-lang.org/rustc/platform-support/android.html)).
Check both pages again after an NDK or Rust toolchain bump.

## Release gates

A green check proves its own row only.

| Claim | Check | It does not prove |
|-------|-------|-------------------|
| Every 64-bit LOAD segment is 16 KiB aligned | `llvm-readelf -lW` on every shipped `.so` | ZIP alignment inside the package |
| Uncompressed native entries are 16 KiB ZIP aligned | `zipalign -c -P 16 -v 4` on the final APK or the default APK set | Native load on a 16 KB device |
| The app loads without 16 KB backcompat mode | Installed smoke on a 16 KB device with backcompat off | Other ABIs |
| The `.so` exports only its boundary | Dynamic-symbol allowlist diff | That the Java names match |
| The library loads and a JNI call works | Installed release smoke from the final APK or AAB | Symbol correlation |
| Crash symbols match the shipped code | GNU build-ID comparison, shipped copy against retained input | Symbol completeness |
| Size stays in budget | Size gate against the checked-in baseline | Speed |

Run the ELF, export, and size gates in one repository script against the merged native-library
tree, and the package and device checks against the final artifact: a check of the merged
`jniLibs` tree proves nothing about the archive that ships. Build the package and its native
symbols in one release invocation. Report a device row with no 16 KB device as not run, never as
passed.

Read [references/elf-verification.md](references/elf-verification.md) when you write or change the
gate script, or when the size gate fails: it has the check commands, the size-gate thresholds,
and the size audit.
Read [references/release-packaging.md](references/release-packaging.md) when you configure native
debug symbols, prove a release closure, run the installed smoke, or publish an AAR or Prefab
package.

Get separate explicit authorization before a Play Console upload, a symbol upload, or a repository
publish. Local builds and inspection need none.

## Failure triage

| Symptom | Likely cause | First check or fix |
|---------|--------------|--------------------|
| Final Android build produced no `.so` | No `cdylib` crate type | Pass `--crate-type cdylib` to `cargo rustc`, or declare it in the manifest |
| `--target armv7a-linux-androideabi` fails | NDK driver name used as the Rust target | Use `armv7-linux-androideabi` |
| Linker not found for `armeabi-v7a` | Driver name without the `a`, or no driver for that API level | `ls "$NDK_BIN" \| grep armv7a`, then recompute `<api>` |
| `dlopen` fails, or the device shows a 16 KB backcompat warning | A 64-bit LOAD segment is `0x1000`, or an uncompressed library sits at a 4 KiB ZIP offset | The ELF check, then the package check |
| The `rustc` line from `cargo build -v` lacks the alignment or build-ID flag | `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` is set, or cargo ran outside the workspace root and did not read the config file | Unset the variable, or run cargo from the workspace root (see [Link flags](#link-flags-in-cargoconfigtoml)) |
| One `.so` in the tree is `0x1000` on a 64-bit ABI | A prebuilt shared library linked without the flag | Rebuild it with a linker option |
| ELF check passes, package check fails | Uncompressed libraries at a 4 KiB ZIP offset | AGP 8.5.1 or later, or `useLegacyPackaging = true` |
| Only a universal APK passes `zipalign` | Split and standalone variants not checked | Build and check the default APK set |
| Build fails with undefined `PAGE_SIZE` | NDK r28 and later do not define it on 64-bit ABIs | Call `sysconf(_SC_PAGESIZE)` |
| Link fails with `version script assignment of 'global' to symbol ... failed: symbol not defined` | A user version script names an absent symbol | Remove the version script |
| Unexpected exported symbol | An `#[unsafe(no_mangle)]` item outside the boundary, maybe in a dependency | Allowlist diff, then remove it at the source |
| `UnsatisfiedLinkError` on a JNI method | Expected `Java_*` export is absent or misnamed | `comm -13` of the allowlist files; the `rust-jni` skill owns naming |
| No build-ID note in the `.so` | `--build-id=sha1` absent for that target | `llvm-readelf -n` and the config tables |
| AGP cannot extract native debug metadata | Cargo stripped the input | `strip = "none"` in the selected profile |
| `.so` grew after a dependency change | New transitive crate, or lost LTO | The selected profile, then `llvm-nm --size-sort` on the unstripped `.so` or `cargo bloat --crates` on a `cdylib` package |
| A panic aborts instead of raising a Java exception | `panic = "abort"`, or the nightly `panic = "immediate-abort"`, in the selected profile | Set `panic = "unwind"` in the selected profile when a guard uses `catch_unwind`; the `rust-panic-safety` skill owns the policy |
| Parallel ABI builds run one after another | A shared `CARGO_TARGET_DIR` | One target directory per ABI |

## Link flags in `.cargo/config.toml`

Put the Android link flags in `.cargo/config.toml` at the workspace root. Cargo reads config
files from the current directory upward, not from the `--manifest-path` directory. Run every
Android cargo invocation from the workspace root or below it, or cargo drops these flags without a
message. Two `cfg` tables cover every Android target:

```toml
[target.'cfg(target_os = "android")']
rustflags = ["-C", "link-arg=-Wl,--build-id=sha1"]

[target.'cfg(all(target_os = "android", any(target_arch = "aarch64", target_arch = "x86_64")))']
rustflags = ["-C", "link-arg=-Wl,-z,max-page-size=16384"]
```

Cargo joins the `rustflags` of every matching `[target.<triple>]` and `[target.<cfg>]` table, so a
profiling addition such as `-C force-frame-pointers=yes` adds to these flags. A matching target
table makes Cargo ignore `build.rustflags`: copy any entry Android builds need into the cfg table.

| Flag | Why |
|------|-----|
| `-Wl,--build-id=sha1` | The NDK driver adds no build ID to a Rust link. Without one, a stripped `.so` cannot be matched to its symbol file. Do not pass a plain `--build-id`: LLD then writes an 8-byte ID that the Android Studio LLDB does not recognize. |
| `-Wl,-z,max-page-size=16384` | Aligns LOAD segments to 16 KiB on `arm64-v8a` and `x86_64`. NDK r28 and later do this by default for those ABIs; the flag keeps the rule explicit and reviewable. On NDK r27 or older, also pass `-Wl,-z,common-page-size=16384`, or upgrade the NDK. |

The NDK links `armeabi-v7a` and `x86` at 4 KiB (`0x1000`) and plans no 16 KiB page size for
32-bit ABIs. Apply the 16 KiB flag there only as a stated policy, and then gate on it.

A `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` environment variable replaces every config `rustflags`
entry; even `RUSTFLAGS=""` silently removes the build ID. Never set or inherit either variable in
the Gradle task or CI job that builds the shipped library. To deny warnings, use `CARGO_BUILD_WARNINGS=deny` (Cargo 1.97+)
instead of `RUSTFLAGS=-Dwarnings`.

## 16 KiB page size

### Policy as of 2026-09

The [page-size guide](https://developer.android.com/guide/practices/page-sizes) (updated
2026-09-16) requires apps that target API 35 or higher to support 16 KB pages on 64-bit devices.
From 2027-02-01, Play blocks an app update that does not. New phone apps and updates must target
API 36 since 2026-08-31
([target API rules](https://developer.android.com/google/play/requirements/target-sdk)), so every
phone release is in scope. Read both pages again before a release decision.

A 16 KB device can run a non-compliant app in backcompat mode, so a misaligned library can load
and a smoke test can pass. Fail the release gate when the merged release manifest sets
`android:pageSizeCompat="enabled"`: a library manifest can add it, and it hides the warning.

### Checks

- **ELF:** run `llvm-readelf -lW` on every shipped `.so` in the merged tree. Expect `0x4000` (or
  a larger power of two) on 64-bit ABIs, and `0x1000` on 32-bit ABIs unless policy says
  otherwise. [references/elf-verification.md](references/elf-verification.md) has the command.
- **Package:** a correctly linked `.so` can still sit at a 4 KiB ZIP offset. Run
  `zipalign -c -P 16 -v 4` (build-tools 35.0.0+) on the final APK, or on every APK of the
  default AAB set, never only a universal APK. Fail when an AAB with uncompressed native
  libraries does not declare `PAGE_ALIGNMENT_16K`.
- **Device:** on a 16 KB device with backcompat off, run the installed release smoke.

Read [references/page-size-checks.md](references/page-size-checks.md) when you run the package
or device check: it has the `bundletool` and `adb` commands and the read-back rules.

### Common traps

Read the elf-verification reference when a prebuilt `.so`, a static C archive, an `mmap` or `mprotect` size, or a C `PAGE_SIZE` macro breaks 16 KiB alignment.

## Release profile

Declare the profiles in the workspace `Cargo.toml`:

```toml
[profile.android-jni]
inherits = "release"
opt-level = "z"             # measure against "s" and 3
lto = "fat"
codegen-units = 1
panic = "unwind"            # catch_unwind at the JNI boundary needs unwind
strip = "none"              # packaging strips the shipped copy
debug = "line-tables-only"

[profile.android-jni-dev]
inherits = "dev"
opt-level = 1               # faster local iteration
panic = "unwind"
debug = "line-tables-only"  # no debugger variables; use true for an LLDB session
```

- Keep `panic = "unwind"` while an export relies on `catch_unwind`: under `"abort"` a panic
  kills the app process instead of raising a Java exception.
- Keep `strip = "none"`. The packaging step strips the shipped copy and extracts symbols from the
  unstripped input; a Cargo-stripped input destroys the symbol file.
- Measure `opt-level`, `lto`, and `codegen-units` changes on the real `.so` with the size gate.
  `"z"` is not always the smallest, and `"s"` and `"z"` disable loop vectorization.
- Do not add `-Wl,--gc-sections`: rustc already passes it for a `cdylib`. Use `-Wl,--icf=all` only
  after a size measurement, and only when no linked C or C++ code compares function addresses.

## Exported symbols

Read `references/elf-verification.md` when you derive `expected-exports.txt`, run the dynamic-symbol allowlist diff, consider a `--version-script`, or remove an unexpected export.

## Gradle, targets, and linker

Build the shipped library through one Gradle task per ABI that writes
`<generated-dir>/jniLibs/<abi>/libnative.so`. Give each ABI its own `CARGO_TARGET_DIR`, pass
`--locked`, remove both rustflags variables from the task environment (the Gradle daemon inherits
them), and set the `Exec` task `workingDir` to the workspace root. Read
[references/ndk-and-gradle.md](references/ndk-and-gradle.md) when you write or change the task, map an Android ABI to its Rust target and NDK driver, add a `rustup` target, set a `CARGO_TARGET_<TRIPLE>_LINKER` variable, run the manual `cargo rustc --crate-type cdylib` build, or compute `<api>`. The
`rust-cross-compilation` skill owns the full cross-compilation variable set.

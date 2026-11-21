# Android Release Packaging

Read this file for the production release closure after the Rust `.so`
passes the alignment, export, and size gates in [SKILL.md](../SKILL.md).

Contents: artifact path; AGP native symbols; stripped and unstripped copies;
exact release closure; installed release smoke; reusable SDK (AAR, Prefab);
external action boundary; triage.

## Choose the artifact path

| Product | Final artifact | Native symbols | Runtime proof |
|---------|----------------|----------------|---------------|
| Play application | AAB | Symbol metadata embedded by AGP, plus retained unstripped inputs | Install APKs generated from the exact AAB |
| Direct APK application | APK | `native-debug-symbols.zip` or an equivalent sidecar | Install the exact APK |
| Kotlin-facing Android SDK | AAR with `jni/<abi>/*.so` | Sidecar retained by the SDK publisher | Build and install a consumer app |
| C or C++-facing Android SDK | AAR with Prefab metadata | Sidecar retained by the SDK publisher | Link, build, and install a consumer app |

Do not create an AAR for an application that already owns the Gradle module.
Use its existing `jniLibs` integration. Do not enable Prefab for a Kotlin-only
API. Prefab exists to publish native headers and libraries to C and C++
consumers.

## Configure Android Gradle Plugin symbols

For Android Gradle Plugin 4.1 and later, select the symbol level in the release
build type:

```kotlin
android {
    buildTypes {
        release {
            ndk {
                debugSymbolLevel = "FULL"
            }
        }
    }
}
```

Use `FULL` when crash reports need file names and line numbers. Use
`SYMBOL_TABLE` when function names are sufficient or the full archive exceeds
the 1.6 GB limit for the native debug symbols file. Keep the Rust profile at
`strip = "none"`. If Cargo strips the input `.so`, AGP cannot recover the
removed DWARF or symbol table.

AGP includes configured native symbol metadata in an AAB. For an APK release,
AGP writes a separate file at this version-dependent path:

```text
<module>/build/outputs/native-debug-symbols/<variant>/native-debug-symbols.zip
```

Treat the pinned AGP documentation and observed variant outputs as the source
of truth. Check them again after an AGP upgrade. Do not hardcode an
`intermediates/` path into a long-lived release job. Intermediate paths are not
a public artifact contract.

Run the release build with Gradle `--info`. Fail when AGP reports that it cannot
extract native debug metadata because an input library is already stripped.
Retain the unstripped Rust output tree as a release artifact even when the AAB
contains symbol metadata. It is the input for local tombstone symbolication and
for proving the release correlation.

## Keep stripped and unstripped copies separate

Use explicit directory names:

```text
release-native/
|-- unstripped/<abi>/libnative.so
|-- packaged/app-release.aab
`-- symbols/native-debug-symbols.zip
```

Do not strip in place. Copy the unstripped output into the Gradle variant input
and let the packaging task create the shipped copy. A manual packaging system
can use `llvm-strip`, but it must write to a different path:

```bash
cp <unstripped>/libnative.so <packaged>/libnative.so
"$NDK_BIN/llvm-strip" --strip-unneeded <packaged>/libnative.so
```

The shipped library should be stripped. The retained input should contain the
symbol information selected by the release policy. Both copies must retain the
same ELF build ID.

Inspect the effective release variant's `jniLibs.keepDebugSymbols` patterns in
the pinned AGP DSL. Fail when any pattern matches a shipped Rust library. A
broad inherited pattern can silently disable AGP stripping. Then extract each
final `.so` from the APK, or from the APK set generated from the AAB, and reject
DWARF sections:

```bash
if "$NDK_BIN/llvm-readelf" -SW <shipped-lib.so> | grep -Eq '[[:space:]]\.debug_'; then
    echo "final native library still contains debug sections" >&2
    exit 1
fi
```

Run this check on the extracted final ELF, not on the merged `jniLibs` input.
Keep the unstripped input and its build ID even after this check passes.

## Prove the exact release closure

Build the final package and symbols in one immutable release job. Record the
package digest, symbol-sidecar digest, pinned NDK version, pinned AGP version,
Cargo lockfile digest, and source revision in the job manifest.

Extract native libraries from the exact final package. AAB native entries are
under module paths such as `base/lib/<abi>/`. APK entries are under
`lib/<abi>/`. If the final artifact is an AAB, also generate the APK set used
for device delivery and inspect its APKs.

For each shipped `(ABI, library)` pair, require exactly one unstripped symbol
input. Extract the corresponding ELF payload from the symbol sidecar when the
selected AGP version emits one. Read the GNU build ID from each ELF:

```bash
"$NDK_BIN/llvm-readelf" -n <shipped-lib.so> | sed -n 's/.*Build ID: //p'
"$NDK_BIN/llvm-readelf" -n <unstripped-lib.so> | sed -n 's/.*Build ID: //p'
"$NDK_BIN/llvm-readelf" -n <sidecar-elf> | sed -n 's/.*Build ID: //p'
```

Fail on a missing ID, duplicate pair, ABI mismatch, library-name mismatch, or
different ID. Also list the sidecar before retention:

```bash
unzip -Z1 native-debug-symbols.zip
```

Require one expected ABI directory and library entry for each shipped native
library that the sidecar claims to cover. Require the sidecar ELF build ID to
match when that payload contains the note. If the pinned AGP version emits a
non-ELF metadata format, record its mapping to the retained unstripped input in
the release manifest. Do not accept the archive-structure check as symbol
compatibility proof. The shipped-to-symbol-input build-ID comparison remains
mandatory.

Do not rebuild a `.so` to recreate lost symbols. A rebuild can produce a
different build ID even at the same source revision. Retain the symbol inputs
from the same build that produced the final package.

The `rust-debugging` skill owns tombstone symbolication; it depends on this
closure because, with NDK r29 or later, `ndk-stack -sym` accepts
`native-debug-symbols.zip` and finds libraries by build ID, not by file name.
Rust frames use v0 mangling (`_R` prefix) since Rust 1.97, and NDK r30
`llvm-cxxfilt` demangles them; `#[unsafe(no_mangle)]` exports keep their names.

## Run the installed release smoke

Test the signed release artifact, not a debug package and not the merged
`jniLibs` directory. Keep one instrumentation test that calls
`System.loadLibrary()` and invokes a stable JNI function with a deterministic
result.

AGP builds an androidTest APK only for the build type that `testBuildType`
names, and the default is `debug`. Set it to the release build type, or to a
release-like build type with the same native build and signing:

```kotlin
android {
    testBuildType = "release"
}
```

Then `./gradlew :<app-module>:assembleReleaseAndroidTest` builds the test APK.
`am instrument` requires the test APK and the app under test to carry the same
signing certificate. Sign both with one key. For an AAB, pass that same key in
`<test-signing-options>` to `bundletool build-apks`.

On a 16 KB device or emulator image, run the device check in
`references/page-size-checks.md` before the install: confirm `getconf PAGE_SIZE` prints
`16384`, turn 16 KB backcompat mode off, and read both properties back.
Otherwise the smoke can pass with a misaligned library.

For a direct APK:

```bash
adb install -r <final-release.apk>
adb install -r <release-androidTest.apk>
adb shell am instrument -w <test-package>/<runner-class>
```

For an AAB, build and install the device-specific APK set from the exact final
AAB:

```bash
bundletool build-apks \
  --bundle=<final-release.aab> \
  --output=<final-release.apks> \
  --connected-device \
  --overwrite \
  <test-signing-options>
bundletool install-apks --apks=<final-release.apks>
adb install -r <release-androidTest.apk>
adb shell am instrument -w <test-package>/<runner-class>
```

Pass signing passwords through the CI secret mechanism. Do not put them on the
command line or in logs. The smoke passes only when the JNI call returns the
expected result. Installation alone does not prove that the library loads.

Run this smoke on one device for every shipped ABI family that CI can exercise.
Use the existing ABI matrix instead of creating a second list. Keep broader API
level coverage in the Android application test policy.

## Publish a reusable SDK only when required

Use an Android library module when another application consumes the Rust
library. Put Kotlin or Java bindings and `jni/<abi>/*.so` in the AAR. Inspect
the final archive and require the complete declared shipping ABI matrix:

```bash
unzip -Z1 <sdk-release.aar> | grep '^jni/[^/]*/libnative\.so$'
```

Enable Prefab only when native C or C++ consumers need headers and linkable
libraries. AGP 4.1 and later can publish libraries that the module builds
through its configured CMake or ndk-build graph:

```kotlin
android {
    buildFeatures {
        prefabPublishing = true
    }
    prefab {
        create("native") {
            headers = "src/main/cpp/include"
        }
    }
}
```

This block does not turn a Rust `.so` copied through `jniLibs` into a Prefab
module. Choose one complete path:

- Represent the Rust artifact in the module's supported external native build
  graph, then use AGP `prefabPublishing`.
- Build the Prefab package layout explicitly and add that complete package to
  the AAR. Keep the Rust cargo task as its only library producer.

Do not enable `prefabPublishing` until one path owns the Rust artifact. An empty
Prefab module with headers but no per-ABI library is a broken SDK.

Inspect the final AAR. Require `prefab/prefab.json`, the expected
`prefab/modules/<module>/module.json`, exported headers, and one library plus
`abi.json` for every shipping ABI below
`prefab/modules/<module>/libs/android.<id>/`. Read each `abi.json` and compare
its `abi`, `api`, `ndk`, `stl`, and `static` fields with the artifact policy. Do
not infer an ABI from the arbitrary `<id>` directory name.

Publish one documented release variant. Build a clean consumer application
against the final AAR or the exact staged Maven coordinate. For Prefab, compile
one C or C++ caller through the consumer's native build. Then install the
consumer release app and invoke the Rust-backed operation. Inspection of the
AAR alone does not prove consumer linkage or runtime loading.

## External action boundary

Creating, inspecting, signing for a local smoke, and retaining release
artifacts are local build actions. Uploading an AAB or symbols to Play Console,
publishing an AAR to a repository, or changing an existing release requires
explicit authorization for that external action. Stop before the upload when
the current request authorizes only build or verification.

## Failure triage

| Symptom | Likely cause | First check |
|---------|--------------|-------------|
| AGP cannot extract native metadata | Cargo or an earlier task stripped the input | Inspect the Rust profile and the pre-packaging `.so` |
| Sidecar contains the expected file but a crash does not symbolicate | Symbols came from another build | Compare GNU build IDs |
| One ABI is absent from the symbols archive | Release ABI set and packaging inputs differ | Diff archive entries against the final package |
| `am instrument` fails with `does not have a signature matching the target` | The androidTest APK and the app APKs carry different certificates | Sign both with one key: same signing config in Gradle, same key in the `bundletool` options |
| Installed app throws `UnsatisfiedLinkError` | Wrong ABI, missing library, or missing JNI export | Inspect installed APKs and the ELF export allowlist |
| AAR consumer links but fails at runtime | Transitive native dependency is absent | Inspect `DT_NEEDED` and every `jni/<abi>/` directory |
| Prefab consumer cannot find or link the Rust library | The Rust `.so` never entered the supported native build graph, or the explicit package lacks metadata or an ABI library | Inspect the final AAR `prefab/` tree and build a clean CMake consumer |

## Official references

- [Include native symbols in a release build](https://developer.android.com/build/include-native-symbols)
- [Native dependencies with the Android Gradle plugin](https://developer.android.com/build/native-dependencies)
- [bundletool](https://developer.android.com/tools/bundletool)

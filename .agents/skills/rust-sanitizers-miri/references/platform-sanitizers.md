# On-Device Sanitizers: Android and iOS

Platform-specific runtime validation for a Rust library that you
cross-compile into a mobile app. The host-side ASan, TSan and MSan flow and
Miri are in `SKILL.md`.

Contents:

1. Android: HWASan
2. Android MTE (Memory Tagging Extension)
3. iOS: Xcode sanitizers and hardware memory tagging

Platform requirements:

| Platform | Tool | Requirement |
|---|---|---|
| Android arm64 | HWASan | Android 14+ with `wrap.sh`; Android 10 to 13 need a HWASan system image |
| Android arm64 | MTE heap checks | Android 13+ on a device whose `/proc/cpuinfo` lists `mte`; set `android:memtagMode` |
| iOS device or Simulator | Xcode ASan | Scheme Diagnostics, or `-enableAddressSanitizer YES` |
| iOS Simulator | Xcode TSan | Scheme Diagnostics, or `-enableThreadSanitizer YES` |
| iPhone and iPad with A19 or later | Hardware memory tagging | Xcode Enhanced Security capability; soft mode (simulated crash report) by default |

The facts below match the NDK HWASan page (updated 2026-03-06), the NDK MTE page
(updated 2026-07-08) and the Xcode Enhanced Security page, read 2026-09-24.
Check the current [Android HWASan guide](https://developer.android.com/ndk/guides/hwasan),
[Android MTE guide](https://developer.android.com/ndk/guides/arm-mte),
[Xcode sanitizer guide](https://developer.apple.com/documentation/xcode/diagnosing-memory-thread-and-crash-issues-early)
and [Xcode Enhanced Security guide](https://developer.apple.com/documentation/xcode/enabling-enhanced-security-for-your-app)
before you change the platform matrix.

## 1. Android: HWASan

Use HWASan to validate a cdylib or staticlib that you cross-compile to Android.

```bash
# HWASan: arm64 only. See the device requirements below.
RUSTFLAGS="-Zsanitizer=hwaddress -Cforce-frame-pointers=yes" \
    cargo +nightly build --locked -Zbuild-std \
    --target aarch64-linux-android
```

Put the flags in `.cargo/config.toml` under the target section if you do not
want to repeat them on every command.

HWASan is the supported Android sanitizer for memory errors. The NDK marks
Android ASan unsupported as of 2023. HWASan needs a 64-bit Arm device. On
Android 14 and later, a debuggable app can start HWASan through `wrap.sh`.
Android 10 through 13 need a HWASan build of Android, such as a supported Pixel
system image.

To run on the device:

1. Build every native target with HWASan and frame pointers.
2. Use `c++_shared` when the app also links libc++. HWASan replaces `new` and
   `delete`, and it cannot do so in a statically linked STL.
3. For Android 14 or later, package this `wrap.sh` for `arm64-v8a`. Remove
   `android:useAppZygote` from the manifest: `wrap.sh` does not work with it.

   ```bash
   #!/system/bin/sh
   LD_HWASAN=1 exec "$@"
   ```

4. For Android 10 through 13, install a compatible HWASan system image.
5. Run the debuggable app and read the report from logcat or the tombstone.

## 2. Android MTE (Memory Tagging Extension)

MTE is available on select arm64 devices from Android 13, for example Pixel 8
and later Pixel phones. Check the device before you configure the app:

```bash
adb shell grep mte /proc/cpuinfo
```

Continue only when the feature list contains `mte`. Some devices support MTE
but do not enable it by default. On those, reboot with MTE from Developer
Options > Memory Tagging Extension. The NDK calls this mode experimental and
not for normal use.

### Heap checks: manifest activation

Heap MTE needs no Rust code change. Set the mode in `AndroidManifest.xml`:

```xml
<application
    android:memtagMode="sync"
    ... >
```

| Mode | Effect |
|---|---|
| `sync` | The CPU stops the process at the faulting access, with `SEGV_MTESERR` and the fault address. Use it in testing. |
| `async` | Lower overhead. The CPU reports the mismatch at a later kernel entry, with `SEGV_MTEAERR` and no fault address. |
| `off` | Explicit disable. |
| `default` | Leave the choice to the platform. Developer Options > App Compatibility Changes can set it per app for an experiment. |

Manifest activation works through the bionic allocator. It checks the native
heap, including Rust allocations through the system allocator, and it needs no
rebuild.

### Stack checks: instrumented rebuild

Stack MTE needs Android 14 QPR3 or later, heap MTE enabled in the manifest as
above, and code built with MTE instrumentation. For the Rust library, that is a
nightly memtag build:

```bash
RUSTFLAGS="-Zsanitizer=memtag -Ctarget-feature=+mte -Cforce-frame-pointers=yes" \
    cargo +nightly build --locked -Zbuild-std \
    --target aarch64-linux-android
```

This Rust recipe is not verified on a device. The NDK recipe also links with
`-fsanitize=memtag -fsanitize-memtag-mode=sync -march=armv8-a+memtag`. rustc
passes none of these link flags (checked on nightly 2026-05-15), so the linked
library can lack the marker that enables stack tagging. For a `cdylib`, you can
pass each of the three flags with `-Clink-arg=` (unverified). Before you count
this as stack coverage, confirm that a deliberate stack out-of-bounds write
produces a tombstone with `SEGV_MTESERR`.

An instrumented build runs only on MTE-capable devices. Use it for debugging,
not for release.

### Cost and detection trade-off

| Setting | Detection | Cost | Use for |
|---|---|---|---|
| `memtagMode="sync"` | Exact report at the access | Higher overhead | Development and soak tests |
| `memtagMode="async"` | Delayed report without the faulting access | Lower overhead | A well-tested release candidate |
| HWASan | Exact report at the access | About 2x CPU, 10% to 35% RAM | A device without MTE, or full stack and heap instrumentation |

Do not use Android ASan as the fallback. It is unsupported. Use HWASan on a
compatible device, or run ASan against a host build.

### Read an MTE crash

```bash
adb bugreport bugreport.zip
unzip -o bugreport.zip 'FS/data/tombstones/*' -d bugreport
grep -rE 'MTEAERR|MTESERR' bugreport/FS/data/tombstones
```

On a production (user) build, `/data/tombstones` is readable only by root or
system, so a direct `adb pull` fails. Pull directly only on a userdebug or
rooted device.

A sync-mode tombstone gives the tagged address and the access kind (read or
write). An async-mode tombstone gives neither: reproduce in sync mode to find
the access. Resolve the Rust frames with `addr2line` against the build that
still carries symbols. See the `rust-debugging` skill; without it, run `ndk-stack` or
`llvm-addr2line` against the unstripped `.so` from the same build.

### What MTE catches

Heap MTE catches use-after-free, double free, and overflows that cross a tagged
heap allocation boundary.

Heap MTE does not catch:

- Stack errors. Those need the instrumented stack build above.
- Reads of uninitialized memory. Use MSan or Miri.
- Data races. Use TSan, Miri with `-Zmiri-many-seeds`, or `loom`.

### Rollout

1. Confirm that `/proc/cpuinfo` reports the `mte` feature.
2. Add `android:memtagMode="sync"` to the debug manifest.
3. Run the full test suite on MTE hardware and fix each report.
4. Test `android:memtagMode="async"` on the release candidate.
5. Confirm that no compatibility failure appears.
6. Ship only after the compatible-device test passes.

## 3. iOS: Xcode sanitizers and hardware memory tagging

### ASan and TSan

Xcode instruments the Swift, Objective-C, and C or C++ code that it compiles. A
prebuilt Rust static library or XCFramework is not instrumented by the scheme
setting. ASan can still report some heap errors in it, because ASan intercepts
the allocator calls. TSan does not check ordinary memory accesses inside an
uninstrumented Rust library. Run ASan and TSan against a supported host Rust
target for that coverage.

Enable ASan in the scheme editor:

```text
Product -> Scheme -> Edit Scheme -> Run -> Diagnostics -> Address Sanitizer
```

Or from the command line:

```bash
xcodebuild \
    -scheme <YourScheme> \
    -destination 'platform=iOS Simulator,name=<Simulator device>' \
    -enableAddressSanitizer YES \
    test
```

Run TSan as a separate Simulator job. Do not enable ASan and TSan in one run.

```bash
xcodebuild \
    -scheme <YourScheme> \
    -destination 'platform=iOS Simulator,name=<Simulator device>' \
    -enableThreadSanitizer YES \
    test
```

### Hardware memory tagging

Apple hardware memory tagging runs on iPhone and iPad with an A19 chip or later,
and on Mac and Apple Vision Pro with an M5 chip or later. On other devices the
setting has no effect. The system tags each allocation and the pointers to it.

Enable it in Signing and Capabilities > Enhanced Security > Memory Safety >
Enable Hardware Memory Tagging. Xcode then also adds the soft-mode entitlement,
so memory tagging runs in soft mode by default: a tag mismatch writes a
simulated crash report and the app keeps running. Deselect "Enable Soft Mode
for Memory Tagging" in the same editor to get a real crash. For more
diagnostics while debugging, also enable Scheme > Run > Diagnostics > Hardware
Memory Tagging.

Rust caveats:

- Apple's page does not mention Rust. Rust heap allocations go through the
  system `malloc` by default, so they should be tagged. Before you count this
  as coverage for the Rust library, confirm that a deliberate use-after-free in
  a debug build produces a (simulated) crash report that names the Rust frame.
- "Enable Enhanced Security for All Targets" also turns on pointer
  authentication and builds for `arm64e`. `arm64e-apple-ios` is a Tier 3 Rust
  target, so this option can block a build that links a Rust library. Enable
  only the memory-tagging option when that happens.
- An Xcode ASan, TSan or memory-tagging pass is not proof that a prebuilt Rust
  library was instrumented. Keep the host sanitizer jobs.

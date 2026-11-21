# Package a Rust staticlib as an XCFramework

Read this when you build the iOS static libraries, write the C header and modulemap, assemble
the XCFramework, or verify its slices.

Contents:

- Produce the static libraries
- Define the C module
- Assemble the XCFramework
- Verify the slices
- Deployment target mismatches

## Produce the static libraries

Keep the normal Rust library type for development. Select `staticlib` in the
packaging command when other consumers still need an `rlib`:

```toml
[lib]
crate-type = ["lib"]

[profile.ios-release]
inherits = "release"
panic = "unwind"            # catch_unwind at the C ABI needs it; rust-panic-safety owns the rule
debug = 1
strip = "none"
```

Export a small C ABI. Keep Rust types, panics, and allocator ownership behind
the boundary:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn native_core_abi_version() -> u32 {
    1
}
```

Build the device and Apple silicon simulator slices from the same lock file,
profile, and deployment target:

```bash
SDKROOT="$DEVICE_SDK" \
  cargo rustc --locked --profile ios-release \
  --target aarch64-apple-ios --crate-type staticlib \
  -p <ffi-crate> --lib

SDKROOT="$SIMULATOR_SDK" \
  cargo rustc --locked --profile ios-release \
  --target aarch64-apple-ios-sim --crate-type staticlib \
  -p <ffi-crate> --lib
```

Build `x86_64-apple-ios` with the simulator SDK only when the support matrix
requires it.

The paths below assume the default target directory. When `CARGO_TARGET_DIR`
or `build.target-dir` is set, read `target_directory` from
`cargo metadata --format-version 1 --no-deps`. Otherwise an old archive under
`target/` can enter the package.

A staticlib does not carry the system libraries and frameworks that its
dependencies link. Print the list after a dependency change:

```bash
SDKROOT="$DEVICE_SDK" \
  cargo rustc --locked --profile ios-release \
  --target aarch64-apple-ios --crate-type staticlib \
  -p <ffi-crate> --lib -- --print=native-static-libs
```

The `note: native-static-libs:` line lists the linker inputs. Xcode links
`-lSystem`, `-lc`, and `-lm` without help. Add each `-framework <Name>` and
each other `-l<name>` to the consumer, for example `-lc++` from a C++ build
script. In SwiftPM, put `.linkedFramework("<Name>")` and
`.linkedLibrary("<name>")` in `linkerSettings` on the Swift target that depends
on the binary target. A `binaryTarget` has no linker settings.

## Define the C module

Generate a header from the exported ABI with the repository's pinned header
generator, or maintain a small header by hand. Do not parse Rust source with a
custom script.

Declare the functions in `native_core.h` with `<stdint.h>` fixed-width types
and `extern "C"` guards for C++ consumers. The header owns the public ABI
version, for example `uint32_t native_core_abi_version(void);`.

Expose the header through a modulemap:

```text
module NativeCore {
  header "native_core.h"
  export *
}
```

Stage the same header and `module.modulemap` beside every library slice. Fail
the package build when generated headers differ from their checked-in form.

## Assemble the XCFramework

When Intel simulator support is required, merge only the two simulator
archives:

```bash
lipo -create \
  target/aarch64-apple-ios-sim/ios-release/libnative_core.a \
  target/x86_64-apple-ios/ios-release/libnative_core.a \
  -output <staging>/simulator/libnative_core.a
```

Otherwise copy the Apple silicon simulator archive to the simulator staging
directory. Keep separate header directories for the two Apple platforms, with
the module files in a subdirectory named after the module. Xcode copies the
headers of every library XCFramework into one `include/` directory, so a
`module.modulemap` at the headers root collides with any other such SDK:

```text
<staging>/device/headers/NativeCore/native_core.h
<staging>/device/headers/NativeCore/module.modulemap
<staging>/simulator/headers/NativeCore/native_core.h
<staging>/simulator/headers/NativeCore/module.modulemap
```

Create the package at a new path:

```bash
xcodebuild -create-xcframework \
  -library <staging>/device/libnative_core.a \
  -headers <staging>/device/headers \
  -library <staging>/simulator/libnative_core.a \
  -headers <staging>/simulator/headers \
  -output <output>/NativeCore.xcframework
```

Use a new versioned or temporary output directory.

## Verify the slices

Read `Info.plist` instead of assuming the generated directory names. Xcode 27
names the slices `ios-arm64` and `ios-arm64-simulator`. A fat simulator slice
has `x86_64` in its name:

```bash
plutil -p <xcframework>/Info.plist
lipo -info <xcframework>/ios-arm64-simulator/libnative_core.a
```

`lipo -info` prints `arm64` for the device and the simulator slice alike. It
proves only the architectures, for example `x86_64` in a fat simulator slice.

Check the platform and minimum OS of every object. Run the check for each
slice:

```bash
otool -l <xcframework>/ios-arm64/libnative_core.a \
  | grep -A3 -E 'cmd LC_(BUILD_VERSION|VERSION_MIN_IPHONEOS)' \
  | grep -E '^ *(platform|minos|version) ' | sort | uniq -c
```

- The device slice shows `platform 2` (iOS) and never `platform 7`.
- The simulator slice shows `platform 7` (iOS Simulator) and never
  `platform 2`.
- An object built for an old minimum, such as a prebuilt `std` object at 10.0,
  has `LC_VERSION_MIN_IPHONEOS` and shows `version` instead of `minos`.
- Every `minos` and `version` value is at or below the app minimum. The
  prebuilt `std` objects keep the rustc default, which is correct.

Use `nm -gU` on a slice to confirm that the public C symbol exists before you
debug Swift imports. Use the Xcode link map when a required Rust symbol is
absent or duplicated.

Link a minimal Swift host. Then inspect the executable that Xcode produced.
`vtool` rejects a static archive with "file is not mach-o", and a static
archive has no dynamic dependency table, so the linked app is the artifact:

```bash
vtool -show-build <DerivedData>/Build/Products/<configuration>-iphoneos/<App>.app/<App>
otool -L <DerivedData>/Build/Products/<configuration>-iphoneos/<App>.app/<App>
```

`vtool` shows the platform, minimum OS, and SDK of the app. `otool -L` shows an
unexpected dynamic dependency or a non-system install name.

## Deployment target mismatches

When the variable is unset, the objects in one slice disagree. rustc builds
Rust objects for iOS 10.0 (device) or 14.0 (arm64 simulator). The `cc` crate
builds C objects for the SDK version, for example 27.0 with Xcode 27. The
linked app still records the app minimum, so `vtool` on the app does not show
the mismatch. The linker prints only a warning, and the C object can call an
API that the minimum OS does not have.

Cargo does not rebuild a crate when only this variable changes (measured with
Cargo 1.98.1). A crate whose `cc` build script reruns is rebuilt. Other crates
can keep their old objects, so one archive can mix minimums. After you change
the value, build in a fresh `CARGO_TARGET_DIR`. The `otool -l` slice check
finds a stale object only when its minimum is above the new value.

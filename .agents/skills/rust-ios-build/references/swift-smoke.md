# Swift consumer and smoke tests

Read this when you add the XCFramework to a Swift package or run the simulator or device smoke
test.

## Integrate with Swift Package Manager

Prefer a local binary target while the package and XCFramework share a
repository:

```swift
.binaryTarget(
    name: "NativeCore",
    path: "Artifacts/NativeCore.xcframework"
)
```

By default, name the binary target after the modulemap module, so the target
and `import` names agree. A local `path` target
has no checksum. Compile a Swift target that depends on the binary target.
Import the module and call `native_core_abi_version()` in a test or smoke
screen.

## Run consumer smoke tests

Run the fastest proof on a macOS CI runner:

1. Build the Apple silicon simulator slice.
2. Assemble the XCFramework.
3. Resolve the local Swift package.
4. Build and run a simulator test that calls the ABI version function.

Pick an installed device and the runtime nearest the minimum iOS version. Pin
the runtime with `OS=`, because the destination defaults to the latest runtime:

```bash
xcrun simctl list runtimes available
xcrun simctl list devices available
xcodebuild test \
  -scheme <smoke-scheme> \
  -destination 'platform=iOS Simulator,name=<available-device>,OS=<runtime-version>'
```

Before release, repeat the call on one physical device at the minimum supported
iOS version when that device is available:

```bash
xcodebuild test \
  -scheme <smoke-scheme> \
  -destination 'platform=iOS,id=<device-udid>'
```

A minimum below iOS 17 needs an Xcode 26.x lane on a macOS 26 host. Xcode 27
cannot run tests on an older device or simulator, and Xcode 26.x does not
support macOS 27. When no such lane exists, report the minimum-OS check as
blocked by the toolchain. Do not report a run on a newer OS as the minimum-OS
result. Do not invent a device identity. Record the device model, OS version,
app version, and test result as release evidence.

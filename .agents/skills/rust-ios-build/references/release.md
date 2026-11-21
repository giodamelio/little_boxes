# Release an iOS Rust artifact

Use these steps after the build checks in `SKILL.md` pass. Each step that signs,
uploads, or publishes needs the user's authorization for that exact action.

Contents: release Xcode, remote SwiftPM archive, XCFramework signing, symbol
preservation, privacy manifest.

## Choose the release Xcode

As of 2026-09, App Store Connect accepts only uploads built with Xcode 26 or
later and the iOS 26 SDK or later
([Apple upcoming requirements](https://developer.apple.com/news/upcoming-requirements/)).
From April 2027, uploads need the iOS 27 SDK or later
([Apple news, 2026-09-09](https://developer.apple.com/news/?id=k1mtkt1k)).
Plan the release Xcode for the date of the upload. Xcode 26 and 27 accept iOS
deployment targets from iOS 15. Xcode 27 supports only iOS 17 or later devices
and simulators
([Xcode system requirements](https://developer.apple.com/xcode/system-requirements/)).
Choose the declared Xcode from these limits.

## Publish a remote SwiftPM binary target

Use a remote target only for a released immutable archive:

```swift
.binaryTarget(
    name: "NativeCore",
    url: "https://example.invalid/NativeCore-<version>.xcframework.zip",
    checksum: "<swift-package-checksum>"
)
```

Sign the XCFramework first when the release policy requires a signed artifact.
Put the XCFramework at the root of the ZIP. Pass `--norsrc` to `ditto`. Without
it, `ditto` stores extended attributes as AppleDouble `._*` files in the ZIP.
SwiftPM extracts the ZIP with `unzip`, so these files enter the consumer's
XCFramework and break its code signature. Then compute the checksum from those
exact bytes:

```bash
ZIP="NativeCore-<version>.xcframework.zip"
ditto -c -k --norsrc --keepParent NativeCore.xcframework "$ZIP"
zipinfo -1 "$ZIP" | grep -c -v '^NativeCore\.xcframework/' || true
zipinfo -1 "$ZIP" | grep -c '/\._' || true
swift package compute-checksum "$ZIP"
```

Both counts must be `0`. The first proves that the XCFramework is the ZIP root.
The second proves that the ZIP has no `._*` file. The `|| true` is necessary
because `grep -c` exits with 1 when the count is 0.

When the XCFramework is signed, verify the tree that SwiftPM extracts before you
publish:

```bash
CHECK_DIR="$(mktemp -d)"
unzip -q "$ZIP" -d "$CHECK_DIR"
codesign --verify --strict --verbose=2 "$CHECK_DIR/NativeCore.xcframework"
```

The checksum is the SHA-256 of the ZIP. A new ZIP of the same XCFramework can
have other bytes, so compute the checksum from the file you upload.

Publish the ZIP and the `Package.swift` checksum as one release operation.
Never reuse a URL for different bytes: every consumer that has the old checksum
then fails to resolve. A checksum mismatch means that the archive or the
manifest changed. Do not bypass the check.

## Sign a distributed XCFramework

Sign only when the release policy requires a signed artifact and the user
authorizes use of the existing identity:

```bash
codesign --timestamp -s "<authorized-identity>" <xcframework>
codesign --verify --strict --verbose=2 <xcframework>
```

## Preserve symbols from the exact release

A Rust static library is not the final load image. Xcode links its object code
into the app executable. Preserve the `.xcarchive` and app dSYM from the same
archive action that produced the distributed app.

Verify the UUID pair before you upload symbols:

```bash
dwarfdump --uuid <archive>/Products/Applications/<App>.app/<App>
dwarfdump --uuid <archive>/dSYMs/<App>.app.dSYM
```

Require the UUID sets to match. Reject a dSYM from a rebuild, even when the Git
revision and version are the same. Preserve the XCFramework ZIP, lock file,
Xcode version, Rust version, app archive, and dSYM under one release identity.

Do not claim that a `.dSYM` made from the `.a` can symbolize the shipped app.
The final Xcode link assigns the app image UUID and addresses. Rust frames get
file and line data only when the Rust profile keeps debug info, as the
`ios-release` profile in `references/packaging.md` does.

## Route privacy manifest work

Do not add an empty `PrivacyInfo.xcprivacy` to every Rust library. First inspect
what the library and its native dependencies do.

- For an app-owned raw static library, record applicable API reasons and data
  practices in the app privacy manifest.
- A static library cannot carry a privacy manifest, and a distributed SDK
  cannot rely on the app's manifest. Wrap the Rust archive in a static
  framework bundle and add `PrivacyInfo.xcprivacy` to it as a resource. Then
  each platform slice of the XCFramework holds a manifest, which is Apple's
  documented route for a binary framework in a Swift package. Put the header in
  `Headers/` and a `framework module NativeCore` modulemap in `Modules/`. Pass
  the bundles to `xcodebuild -create-xcframework` with `-framework` instead of
  `-library` and `-headers`.
- Apple documents `resources: [.process("PrivacyInfo.xcprivacy")]` for a Swift
  package source target, not as the manifest of a binary target. Before you rely
  on a wrapper target to carry the SDK manifest, archive a consumer app and
  confirm the manifest in the Xcode privacy report.
- If the Rust library has no applicable API use or data collection, record the
  audit result. Do not invent declarations.
- Re-run the audit after a native dependency or Apple requirement changes.

The application or SDK package owner owns the manifest. Route policy questions
to the release owner and to current Apple documentation
([privacy manifests](https://developer.apple.com/documentation/bundleresources/adding-a-privacy-manifest-to-your-app-or-third-party-sdk)).
Do not infer a reason code from a linked symbol alone.

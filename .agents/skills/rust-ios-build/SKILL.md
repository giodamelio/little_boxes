---
description: Use when building, packaging, or verifying Rust for iOS as a static library - device and simulator slices, IPHONEOS_DEPLOYMENT_TARGET, C header and modulemap, XCFramework assembly, SwiftPM binaryTarget, dSYM UUID checks, smoke tests, code signing, or PrivacyInfo.xcprivacy. Triggers on aarch64-apple-ios-sim, target_env = "sim", xcodebuild -create-xcframework, swift package compute-checksum, or the ld message built for newer iOS version than being linked.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-ios-build
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 6fe2402bc3620600793f4e3aa35c4bf0ab7a476e
name: rust-ios-build
---
# Rust iOS Build

Build a Rust `staticlib`, expose a C header, package device and simulator
slices as an XCFramework, and prove that a Swift consumer links and runs it.

Route neighbouring work to these skills, when they are installed:

- `uniffi-packaging-versioning` generates the UniFFI header, modulemap, and
  Swift sources with the in-crate `uniffi-bindgen`, never a global one.
- `rust-swift-ffi` owns the hand-written Swift wrapper; a Rust callback hops to
  the main actor with `Task { @MainActor in ... }`.
- `rust-unsafe` owns ABI layout; give each opaque handle its own `#[repr(C)]` type.
- `rust-panic-safety` owns panic containment at the exported C ABI and the
  `panic` profile setting.
- `rust-native-linking` owns `build.rs`, C dependencies, and linker inputs.
- `rust-debugging` symbolicates a crash with `atos` and the matching dSYM.

Link one Rust `staticlib` into an app. Each staticlib carries its own copy of
`std`, and the Rust Reference says that multiple Rust staticlibs "are likely to
conflict". Put every Rust crate the app needs behind one FFI crate.

Do not add an Objective-C wrapper when Swift can import the C header directly.
Do not build a dynamic library for an app when a static library is sufficient.

## Verification

| Claim | Check | When | Does not prove |
| --- | --- | --- | --- |
| Rust code compiles for iOS | `cargo check --locked --target aarch64-apple-ios` | While iterating | Link, package, or load path |
| Each slice has the right platform and minimum OS | `otool -l` slice check in `references/packaging.md` | Every package build | That Swift can import the module |
| The public C symbol exists | `nm -gU` on each slice | Every package build | That the header matches the symbol |
| Swift imports, links, and calls Rust | Simulator smoke test | Every package build | Device architecture, signing, entitlement, or loader behavior |
| The app records the right platform, minimum OS, and SDK | `vtool -show-build` and `otool -L` on the app | Release candidate | The minimum OS of objects inside the archive |
| The device load path and signing work | Physical-device smoke test | Release candidate | Behavior on other OS versions |
| The dSYM belongs to the shipped app | `dwarfdump --uuid` on the app and its dSYM | Release archive | Rust line data, which needs debug info in the Rust profile |

Fail the release when a required target is absent, a header differs between
slices, a row above fails, the physical-device result is missing from the
release evidence, or the app and dSYM UUIDs differ.

Report signing, device access, and privacy review as separate external gaps,
not as Rust build failures. Do not weaken an artifact check to make such a gap
look green.

## Failure triage

| Symptom | Likely cause | Action |
| --- | --- | --- |
| `ld: building for 'iOS-simulator', but linking in object file (...) built for 'iOS'` | A device archive entered the simulator slice | Keep device and simulator archives as separate XCFramework inputs. |
| `ld: warning: object file (...) was built for newer 'iOS-simulator' version (27.0) than being linked (17.0)` | `IPHONEOS_DEPLOYMENT_TARGET` was unset or stale for one object, often a `cc` build | Export the variable, rebuild in a fresh target directory, and repeat the `otool -l` slice check. Treat this warning as a failure. |
| `Undefined symbols for architecture arm64` names the exported function | Header, symbol name, or archive is stale | Run `nm -gU`, compare the header, and rebuild all slices from one revision. |
| Undefined `_Sec*`, `_CF*`, or C++ runtime (`__ZNSt...`, `___cxa_*`) symbols from a Rust dependency | A dependency needs a system framework or library, and a staticlib does not carry that link request | Run the `--print=native-static-libs` build and link each listed framework and library. |
| `duplicate symbol '__R...'` in two `lib*.a` archives | Two Rust staticlibs in one app; `-all_load` or `-force_load` loads both `std` copies | Build one staticlib for the app. |
| Swift reports `no such module` | Header or `module.modulemap` is absent from one slice | Inspect the selected slice and stage the same module files in every headers directory. |
| `Multiple commands produce '.../include/module.modulemap'` | Two library XCFrameworks put `module.modulemap` at the headers root | Stage the module files under `headers/<ModuleName>/`. |
| SwiftPM checksum does not match | ZIP bytes changed after checksum generation | Recreate the immutable release ZIP and update its checksum atomically. |
| Simulator passes and device fails to link | Release lacks the device slice | Require `aarch64-apple-ios` in the release matrix. |
| Crash report stays unsymbolicated | dSYM came from another archive action | Compare executable and dSYM UUIDs with `dwarfdump --uuid`. |
| Device build asks for a team or profile | Signing is not configured or not authorized | Report the signing gap. Do not change account state. |
| Store validation reports privacy-manifest issues | An applicable API or SDK declaration is absent | Audit the current Apple requirements and update the owning package manifest. |

## Respect signing and publishing authority

Building a Rust archive and an XCFramework does not require code signing.
Archiving an app and installing it on a physical device with the existing team,
bundle identifier, certificate, and profile are local actions. They need no
separate authorization.

Get authorization for the exact action before you sign a distributed
XCFramework, register a device, change the development team, create, import,
replace, or revoke a signing identity, profile, or certificate, notarize, or
upload to a store. These actions use release credentials or change account
state.

- Do not enable automatic signing or pass `-allowProvisioningUpdates` to make a
  build pass without authorization. That flag lets `xcodebuild` create
  profiles, app IDs, and certificates on the Apple Developer website.
- Do not print certificate, account, or profile secrets.
- Stop after the requested artifact or test. Do not submit it automatically.

## Select the targets

Use the Tier 2 targets that Rust distributes through `rustup`:

| Rust target | Apple platform | Required use |
| --- | --- | --- |
| `aarch64-apple-ios` | iPhone and iPad device | Release |
| `aarch64-apple-ios-sim` | Apple silicon simulator | Development and CI |
| `x86_64-apple-ios` | Intel simulator | Only when a consumer runs an x86_64 simulator |

Xcode 27 runs only on Apple silicon Macs. An x86_64 simulator now means an
Intel Mac with Xcode 26 or earlier, or a simulator under Rosetta. Install only
the targets in the support matrix:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

Do not detect the simulator from `target_arch = "x86_64"`. An Apple silicon
simulator is `aarch64`. Use the simulator environment cfg:

```rust
pub const IS_APPLE_SIMULATOR: bool = cfg!(all(
    target_vendor = "apple",
    target_os = "ios",
    any(target_env = "sim", target_abi = "sim")
));
```

Rust 1.78 and later set `target_abi = "sim"`; Rust 1.91 and later also set
`target_env = "sim"` (Rust 1.99.1 prints both). The `any(...)` form holds for
every MSRV from 1.78. Below 1.78, `cfg(target_abi)` is unstable: read `TARGET`
in `build.rs` and emit a custom cfg.

Keep platform differences at the host boundary. Do not fork domain logic for
device and simulator builds.

## Pin the Xcode inputs

Build Apple targets on macOS with the Xcode version that the project declares.
Record that version in CI output:

```bash
xcodebuild -version
xcode-select -p
```

Read [references/release.md](references/release.md#choose-the-release-xcode) when you
choose the declared Xcode. It has the App Store SDK minimums.

Resolve the SDK with `xcrun`. Do not check an absolute SDK path into Cargo
configuration:

```bash
DEVICE_SDK="$(xcrun --sdk iphoneos --show-sdk-path)"
SIMULATOR_SDK="$(xcrun --sdk iphonesimulator --show-sdk-path)"
```

## Set one deployment target

Export one `IPHONEOS_DEPLOYMENT_TARGET` for every cargo invocation, equal to the Swift package or
Xcode target minimum; `rustc --print deployment-target --target aarch64-apple-ios` prints the
value rustc uses. When it is unset, Rust and `cc` objects in one slice get different minimums,
and a C object can call an API the minimum OS lacks. Cargo does not rebuild a crate when only this
variable changes (Cargo 1.98.1): after a change, build in a fresh `CARGO_TARGET_DIR`.

## Produce, package, and verify

- Keep `crate-type = ["lib"]` and select `staticlib` with `cargo rustc --crate-type staticlib`.
  Use a profile with `panic = "unwind"` (for `catch_unwind` at the C ABI), `debug = 1`, and
  `strip = "none"`.
- Build the device and simulator slices from the same lock file, profile, and deployment target,
  with `SDKROOT` from `xcrun`. Never merge a device archive with a simulator archive: both can
  contain `arm64`, but they are different Apple platforms. `lipo` merges only the two simulator
  archives.
- A staticlib does not carry the frameworks its dependencies link. After a dependency change,
  print them with `-- --print=native-static-libs` and add each to the consumer.
- Generate the C header with the repository's pinned generator, or keep a small one by hand.
  Stage the header and `module.modulemap` under `headers/<ModuleName>/` beside every slice: a
  modulemap at the headers root collides with other SDKs.
- `xcodebuild -create-xcframework` refuses an existing output; use a new path, and do not delete
  a path the current build did not create.
- Verify every slice with the `otool -l` check: the device slice shows only `platform 2`, the
  simulator slice only `platform 7`, and every `minos` is at or below the app minimum.

Read [references/packaging.md](references/packaging.md) when you build the slices, write the
header and modulemap, assemble the XCFramework, or run the slice checks.

## Integrate and smoke-test

Prefer a local SwiftPM `.binaryTarget` named after the modulemap module while the package and
XCFramework share a repository. Run a simulator test that calls the ABI version function on every
package build, pinned to the runtime nearest the minimum iOS with `OS=`. Before release, repeat it
on a physical device at the minimum iOS. A minimum below iOS 17 needs an Xcode 26.x lane; without
one, report the minimum-OS check as blocked, never as a run on a newer OS.

Read [references/swift-smoke.md](references/swift-smoke.md) when you wire the Swift package or run
the smoke tests. Read [references/release.md](references/release.md) when you publish a remote
binary target, sign the XCFramework, preserve release symbols, or decide where a privacy manifest
belongs.

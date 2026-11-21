# iOS native debugging

Everything here needs Xcode and an iOS build of the Rust library. Work through it only after the
host reproduction in [SKILL.md](../SKILL.md) has failed. When only the Rust library is under
suspicion, skip Xcode: run the same code from a host CLI under `rust-lldb`.

## Crash symbolication

Frames from a Rust `staticlib` linked into the app appear unsymbolicated in Xcode Organizer or a
crash-reporting service. Symbolicate against the `.dSYM` that the app build produced:

```bash
# One address from a crash report; -l is the load address of the image
atos -arch arm64 -o MyApp.app.dSYM/Contents/Resources/DWARF/MyApp \
    -l <load_address> <crash_address>
```

The `.dSYM` is the only reliable input, because the linker moves the archive code into the app
binary. Check that `dwarfdump --uuid` of the `.dSYM` matches the binary image UUID in the crash
report.

The `.dSYM` has Rust file and line only when the staticlib was built with the release debug
setting from the *Debug info* part of SKILL.md. Check it:
`dwarfdump --debug-info MyApp.app.dSYM | grep -m1 '/@/my_ffi\.'` must print a compile unit of your
crate. Do not grep `DW_LANG_Rust`: the std units always match.

## Build for iOS

```bash
cargo build --locked -p my-ffi --target aarch64-apple-ios       # device
cargo build --locked -p my-ffi --target aarch64-apple-ios-sim   # simulator (Apple silicon)
```

## LLDB via Xcode

1. Open the app project or the SwiftPM package in Xcode. Breakpoints in Rust resolve through
   the DWARF in the linked slice, also for an XCFramework binary target.
2. Load the Rust formatters in the LLDB console (Debug > Activate Console), because Xcode does
   not run `rust-lldb`. [rust-gdb-pretty-printers.md](rust-gdb-pretty-printers.md) has the
   command.
3. Run on a device or the simulator with the debugger attached, then break in Rust:

```text
(lldb) b my_crate::module::function_name
(lldb) b rust_panic
(lldb) thread backtrace all
```

## Sanitizers in Xcode

The Xcode Address Sanitizer and Thread Sanitizer switches instrument only the Swift, C, and
Objective-C code that Xcode compiles. They do not instrument a prebuilt Rust `staticlib`, so ASan
checks no Rust memory access: a Rust out-of-bounds read or use after free goes unreported. ASan
still intercepts `malloc` and `free` for the whole process. It therefore catches a double free or
an invalid free on the Rust side when Rust uses the default `System` allocator. Run ASan or TSan
on a host target for Rust coverage, with `RUSTFLAGS=-Zsanitizer=address cargo +nightly test
--target <host-triple>` (the `rust-sanitizers-miri` skill).

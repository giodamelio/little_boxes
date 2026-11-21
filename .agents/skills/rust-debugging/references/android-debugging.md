# Android native debugging

Everything here needs a device or an emulator and the NDK. Work through it only after the
host reproduction in [SKILL.md](../SKILL.md) has failed, because every step costs more
than the same step on the host.

Contents: logcat filtering, tracing to logcat, `RUST_BACKTRACE` on a device, tombstones,
symbolication, device builds, LLDB via Android Studio.

## Logcat filtering

```bash
# Your own native tags (the tags that the bootstrap passes to the logcat sink)
adb logcat -d -s my-native-tag:V my-other-native-tag:V

# The crash buffer: the native crash dump, no root needed. -d dumps and exits.
adb logcat -d -b crash

# Panic records and fatal signals, regardless of tag. rust_panic is the tag that the
# redacted panic hook writes; replace it with the tag of your hook.
adb logcat -d | grep -E "rust_panic|Abort message|SIGABRT|SIGSEGV|SIGBUS"

# Interactive only: without -d, logcat streams until Ctrl-C. Do not run it from a script.
adb logcat -s my-native-tag:V

# Native crash and Java runtime channels.
# Quote the specifiers: an unquoted "*" is a glob in the shell.
adb logcat -d -s 'AndroidRuntime:E' 'DEBUG:*' 'libc:*'
```

An app process sends stdout and stderr to `/dev/null`. `println!`, `dbg!`, and the default
panic hook print nothing to logcat. Until a logcat sink is wired, the crash channels give a
signal and an address, not a panic record.

## Route tracing to logcat

In the bootstrap `cdylib`, call `tracing::subscriber::set_global_default`, then
`tracing_log::LogTracer::init()`. Do not call `android_logger::init_once`,
`SubscriberInitExt::init()` or `try_init()`, or the `fmt` builder's `init()` there. A process
holds one `log` logger: the extra call fails, and `init()` panics. The `rust-observability`
skill, when it is installed, owns this sequence and its reasons (section *One dispatcher
install, multiple sinks*).

A shipped build sends the redacted record to the platform sink. `tracing-logcat` writes the full
rendered event, the message and the field values included. Add it only in a debug or opt-in
build.

Prove the wiring on a device: one `log::info!` from a dependency and one `tracing::info!` must
both reach logcat, and the init path must return success.

For a run-time level override, build the filter on `tracing_subscriber::reload::Layer`. It is
the supported way to change a filter after the subscriber is global. Export a set function and a
clear function from the bootstrap, so you can raise one scope without a rebuild. A raised level
reaches `log` records only because `LogTracer::init()` set `log::max_level()` to `Trace`.

A ring-buffer layer next to the logcat layer lets the app attach the last N events to a bug
report. It gets logs from a user who cannot run `adb`. Apply the redaction rules of the
`rust-observability` skill to every event that it keeps.

## RUST_BACKTRACE on Android

`RUST_BACKTRACE` is **not inherited** by an Android app process. Setting it in your shell, in
Gradle, or in the run configuration does nothing.

1. In an app, use the bootstrap's redacted panic record (the `rust-panic-safety` skill; a
   `std::panic::set_hook` that logs only `info.location()`) and the symbolicated tombstone. Capture a full backtrace only in a local diagnostic build whose
   output does not enter telemetry.
2. For a standalone test binary pushed to the device, the environment works:

   ```bash
   cargo test --locked --no-run --target aarch64-linux-android -p my-crate
   # Push the path that the "Executable ... (<path>)" line prints; do not infer its directory
   adb push <executable-path> /data/local/tmp/my_test
   adb shell "cd /data/local/tmp && chmod +x my_test && RUST_BACKTRACE=1 ./my_test"
   ```

   The path moves with `build.build-dir`. In a script, read the `executable` field of
   `cargo test --no-run --message-format=json`.

## Tombstones

```bash
# All tombstones, with the rest of the system state, in one archive
adb bugreport bugreport.zip

# On a rooted or userdebug device you can also read them directly
adb shell ls -lt /data/tombstones/ | head -5
adb pull /data/tombstones/tombstone_00
```

Read a tombstone in this order:

1. `signal` line. 6 (SIGABRT) is an abort, including a panic that hit an unguarded export.
   11 (SIGSEGV) is a memory fault. 7 (SIGBUS) is usually misalignment.
2. `Abort message` line. It is present only when the aborting code set it: Rust
   `panic = "abort"` or a bionic fatal error. Rust copies a `&str` or `String` panic payload
   into it through `android_set_abort_message` (in `panic_abort` as of Rust 1.99.1), whatever
   the panic hook does. Every crash report that uploads the tombstone carries that text, so the
   panic message must hold no user data. Treat the line as sensitive. With `panic = "unwind"`, a panic that reaches an unguarded
   export aborts with no `Abort message`; look for `core::panicking::panic_cannot_unwind` in the
   symbolicated backtrace.
3. `backtrace:` block. Find the first frame whose path ends in your `.so`, for example
   `#03 pc 0000000000012345  /data/app/.../lib/arm64/libmy_ffi.so (BuildId: 1a2b...)`. The `pc`
   value is the offset inside that library; feed it to `llvm-addr2line`. Keep the `BuildId`.
   A library that loads straight from the APK shows as `base.apk!libmy_ffi.so (offset 0x...)`;
   the `pc` value is still relative to the library.

## Symbolicate

```bash
# The NDK addr2line. The command substitution expands the host glob;
# a plain assignment keeps the "*" literal.
ADDR2LINE=$(echo "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/bin/llvm-addr2line)

# -C demangles (v0 too), -f prints the function, -i expands inlined frames, -e names the binary
$ADDR2LINE -Cfie target/aarch64-linux-android/<ship-profile>/libmy_ffi.so 0x12345 0x67890

# Or symbolicate a whole tombstone or logcat capture
"$ANDROID_NDK_HOME"/ndk-stack -sym target/aarch64-linux-android/<ship-profile> -dump tombstone_00
```

`<ship-profile>` is the profile that built the library Gradle packaged, for example `android-jni`
in the `rust-android-build` skill. A `--release` build of the same crate is a different binary.

`ndk-stack` starts to parse at the `*** *** ***` line; keep that line when you copy a trace.

The `.so` must be the exact build that ran on the device, and it must not be stripped. Gradle
strips the packaged `.so`, so symbolicate against the Cargo output in
`target/<triple>/<ship-profile>/`, not against the copy from the APK. When the frame shows a
`BuildId`, compare it with the `Build ID` that `llvm-readelf -n libmy_ffi.so` prints. A mismatch
means a different binary, and every line it gives you is wrong. A Rust `cdylib` has no Build ID
unless the link adds `--build-id=sha1`; the `rust-android-build` skill owns that flag, stripping,
and symbol upload.

## Build for a device

```bash
cargo build --locked -p my-ffi --target aarch64-linux-android            # debug
cargo build --locked -p my-ffi --target aarch64-linux-android --profile <ship-profile>
```

## LLDB via Android Studio

1. Open the Android project in Android Studio.
2. Run > Edit Configurations > Debugger tab > Debug type: **Dual (Java + Native)**.
3. Add a symbol directory that points at the Cargo output for the device ABI, for example
   `target/aarch64-linux-android/debug/`.
4. Add the `command script import` line for `lldb_lookup.py` to the LLDB startup commands, so
   Rust values print as Rust. See [rust-gdb-pretty-printers.md](rust-gdb-pretty-printers.md).
5. Set breakpoints in the Rust source files and run with the debugger attached. Studio pushes
   and starts `lldb-server` on the device for you.

If breakpoints stay unresolved, the symbol directory is wrong or the packaged `.so` does not
match the one in it. Rebuild both from the same commit.

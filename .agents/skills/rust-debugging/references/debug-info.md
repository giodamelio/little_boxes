# Debug info for debuggers and symbolication

Deep material for `rust-debugging`, section 1. Read it when the debugger shows no variables, a
backtrace has no file or line, or you archive symbols for a release.

- If `[profile.dev]` sets `debug = "line-tables-only"` or `debug = false`, the debugger shows no
  variables. Override it for one build: `CARGO_PROFILE_DEV_DEBUG=true cargo build --locked -p my-cli`.
- A release build has no debug info by default, so a device backtrace has no file or line.
  Turn it on for the artifact that you archive:

```toml
[profile.release]
debug = "line-tables-only"   # file:line for symbolication; use true to inspect variables
strip = false                # keep symbols in the artifact that you archive
```

- On macOS, Cargo defaults to `split-debuginfo = "unpacked"`: the DWARF stays in the object files
  under `target/`, and the binary alone has no line info. To archive a macOS host binary, build
  with `CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO=packed` and archive the `.dSYM` next to it.
- Verify the archive. On ELF, `llvm-readelf -S libmy_ffi.so | grep debug_line` prints a section.
  On Apple, `dwarfdump --uuid` prints the same UUID for the binary and its `.dSYM`.

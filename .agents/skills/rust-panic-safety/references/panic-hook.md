# Privacy-safe panic hook

Read this file when you install or review a panic hook. It holds the catalog's single copy of
the report block; other skills point here.

A guard that returns the panic code with no other signal turns a bug into a mystery. Install a
hook that emits one bounded structured record. Do not inspect the panic payload in shipped code:
it can contain input data, paths, identifiers, or secrets. Rules:

- Let the application-owned outermost Rust FFI bootstrap own `set_hook`. An embedded component
  exposes a redacted handler and never replaces an unknown process-global hook.
- Do not chain the default hook in a shipped embedded process. It formats the payload and file
  path. Keep it only in a local host binary whose stderr is not forwarded to telemetry.
- Map the file path to a closed site code. Emit only that code plus the bounded numeric line and
  column. Do not format `PanicHookInfo`, emit a file path, or capture a backtrace into a shipped
  platform log.
- The hook must not panic. A panic inside the hook aborts the process with `thread panicked
  while processing panic. aborting.` (Rust 1.99.1).
- Install the hook before any task or thread starts. A panic that crosses a task boundary loses
  its location, and only the hook still has it.
- `PanicHookInfo` needs Rust 1.81. Below MSRV 1.81, the hook argument is `PanicInfo`, with the
  same `location()`.

```rust
#[derive(Clone, Copy)]
enum PanicSite {
    Boundary,
    Engine,
    Unknown,
}

fn classify_site(file: &str) -> PanicSite {
    // Cargo passes a workspace member as `<member-dir>/src/...` and a registry
    // dependency as an absolute path, so match a segment, not a prefix. The path
    // keeps the separator of the build host: a Windows build gives `src\boundary\`.
    if file.contains("src/boundary/") || file.contains(r"src\boundary\") {
        PanicSite::Boundary
    } else if file.contains("src/engine/") || file.contains(r"src\engine\") {
        PanicSite::Engine
    } else {
        PanicSite::Unknown
    }
}

pub fn report_panic(info: &std::panic::PanicHookInfo<'_>) {
    let (site, line, column) = info
        .location()
        .map(|location| {
            (
                classify_site(location.file()),
                location.line(),
                location.column(),
            )
        })
        .unwrap_or((PanicSite::Unknown, 0, 0));

    write_platform_panic("rust_panic", site, line, column);
}

// The application-owned outermost Rust FFI bootstrap owns the process-global
// hook and statically composes every component handler once during startup.
pub fn install_bootstrap_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        report_panic(info);
        report_other_library_panics(info);
    }));
}
```

- `write_platform_panic` stands for the platform sink. The `rust-observability` skill covers
  the sink; the hook emits only bounded structured fields and never formats `PanicHookInfo`,
  its payload, or a backtrace.
- `report_other_library_panics` stands for the redacted handlers that embedded components
  expose. The bootstrap composes them here once.
- The bootstrap can install from its `JNI_OnLoad` or from one explicit init export, after it
  composes the component handlers.
- For a message or a backtrace, use `RUST_BACKTRACE=full` in a local host repro. Symbolicate a
  tombstone or crash report offline against the exact unstripped binary for an app process.

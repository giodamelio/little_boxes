# Panic Strategy

Extended reference for the `rust-panic-safety` skill. Read this when you set or review the
`panic` key of a profile, or when a guard must keep working in the shipped build.

## Unwind or abort

- `panic = "unwind"` (default) keeps panics catchable; landing pads add code. Use it when a
  panic must be survived: a shared library inside a host process, a server that isolates a
  request, any FFI boundary that returns an error to the caller.
- `panic = "abort"` aborts at the panic site. It is smaller and marginally faster. Use it when
  no guard depends on catching, for example a standalone binary where a panic is a crash
  anyway. It silently turns every `catch_unwind` guard into dead code.

```toml
[profile.release]
panic = "unwind"                # required if any entry point relies on catch_unwind
overflow-checks = true          # a wrapped length becomes a panic, not a bad slice bound
debug = "line-tables-only"      # file and line when you symbolicate a backtrace offline
```

Rules:

- Cargo reads manifest profiles only from the workspace root. A `[profile]` table in a member
  manifest has no effect, and a per-package override cannot set `panic`. A config-file
  profile, `CARGO_PROFILE_<NAME>_PANIC`, and `-C panic` in any rustflags source override the
  manifest.
- Cargo ignores the `panic` key for tests, benchmarks, build scripts, and proc macros. A
  `#[should_panic]` test therefore runs under unwind even when release aborts. Do not read a
  green test run as proof that the shipped library unwinds.
- State the reason for the strategy in a comment next to the `panic` key.

## Check the shipped strategy

Run the `--print cfg` check with the same `--target`, `--profile`, and environment as the
shipping build. It then reflects every source of the strategy: the root manifest profile,
`[profile.<name>]` in `.cargo/config.toml`, `CARGO_PROFILE_<NAME>_PANIC`, `RUSTFLAGS`,
`CARGO_ENCODED_RUSTFLAGS`, `build.rustflags`, and `target.<triple>.rustflags`. Without
`--target`, Cargo builds for the host and skips the rustflags of the shipping triple, so the
check can print `unwind` for a library that ships with `abort`. Replace `--release` with
`--profile <name>` when the shipping profile is not `release`.

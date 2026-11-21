# Verification commands

Read this when you write the host-test, target-build, CAS, ELF-inspection, probe, or
hardware-in-the-loop checks for firmware or a `no_std` library.

Contents:

- Host and target builds
- Libraries that claim cores without CAS
- ELF inspection
- Probe and hardware-in-the-loop

## Host and target builds

Keep protocol parsing, state machines, unit conversion, filters, and retry
decisions independent of registers and global peripherals. Test that library on
the host. Replace a peripheral with a small trait fake only at the existing HAL
boundary; do not emulate the whole chip.

```bash
host_target="$(rustc -vV | sed -n 's/^host: //p')"
cargo test --locked -p <logic-crate> --lib --target "$host_target"
cargo check --locked --target <target> --no-default-features --features <chip-feature>
cargo build --locked --release --target <target> --bin <firmware>
```

Host tests prove logic only. They prove nothing about startup, registers,
timing, or interrupts. A clean target build proves that the image links inside
the declared regions; it does not prove stack or heap limits at run time. Do not
put a global `build.target` in a reusable workspace without a host-test command
like the one above: it can make `cargo test` try to execute firmware on the host.

Use `--all-features` only when the features are additive. When features select
a chip or runtime, check each supported feature set explicitly, and reject
invalid combinations with `compile_error!`.

## Libraries that claim cores without CAS

When a library claims support for cores without compare-and-swap (CAS), add this
check to its gate even if the product ships on a CAS-capable core. Run
`rustup target add thumbv6m-none-eabi` first.

```bash
cargo check --locked -p <lib> --target thumbv6m-none-eabi --features portable-atomic/critical-section
```

The backend feature belongs to this command only, never to the library's
manifest. `cargo check` does not link, so it needs no `critical-section`
implementation. Without the flag, a library that needs CAS fails this check as
expected; do not fix that by adding a backend to its `Cargo.toml`.

## ELF inspection

Inspect the final ELF. These `cargo-binutils` wrappers need
`rustup component add llvm-tools`. If the workspace does not install them, use
the matching LLVM tools from the pinned toolchain; do not add tools only to make
command spelling uniform.

```bash
cargo size --release --target <target> --bin <firmware> -- -A
cargo objdump --release --target <target> --bin <firmware> -- -h
cargo nm --release --target <target> --bin <firmware> -- --size-sort
```

## Probe and hardware-in-the-loop

Install the probe tools with `cargo install probe-rs-tools --locked`. It provides
`probe-rs`, `cargo-flash`, and `cargo-embed`. Give `probe-rs` an explicit chip.
Do not rely on automatic chip detection in CI or production instructions.

```bash
probe-rs run --chip <chip> target/<target>/release/<firmware>
probe-rs attach --chip <chip> target/<target>/release/<firmware>
```

`probe-rs attach` does not reset or flash, so it preserves a running fault.

Add hardware-in-the-loop checks only for behavior that a host cannot prove:
startup, interrupt routing, clock accuracy, DMA, reset cause, watchdog action,
sleep and wake, flash update, and real peripheral timing. Give each check a
timeout and a recovery action so a failed board does not occupy CI forever.

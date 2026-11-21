# Board bring-up, target selection, and memory layout

Use these rules when you bring up a board, port to a new chip, change the boot
path or memory layout, own a linker script, or configure the runner and link
arguments.

Contents:

- Inspect the workspace and select a target
- Target contract
- Built-in and custom targets
- Make `memory.x` visible to the linker
- Memory file
- Linker-script invariants
- Runner and link arguments

## Inspect the workspace and select a target


Inspect the current workspace:

```bash
grep -rnE --include='*.rs' --exclude-dir=target \
  '#!\[(no_std|no_main)\]|entry|interrupt|exception|global_allocator|panic_handler' .
grep -rnE --include='*.toml' --include='*.json' --include='*.x' --exclude-dir=target \
  'memory\.x|link\.x|runner|rustflags|build-std|panic[[:space:]]*=' .
cargo tree --edges normal,build
rustc --print target-list | grep -E '<architecture>|thumb|riscv'
```

Use a built-in target when it matches the CPU, ABI, floating-point mode, and
atomic support. Install it with `rustup target add <target>`. Then read
`rustc --target <target> --print cfg`: it is the source of truth for CAS
support, pointer width, endianness, and target features. Since Rust 1.95 a
custom JSON target spec needs a pinned nightly; add one only when no built-in
target fits.

Do not infer the memory map from a similar board. Account for a bootloader,
configuration pages, secure regions, and retained RAM explicitly.

Prefer the architecture runtime crate and its linker template. Supply the chip
memory regions in `memory.x` or the runtime's documented input. Do not copy the
runtime's full linker script into the application unless the shipped image has
a requirement that its extension points cannot express. Add linker `ASSERT`
expressions for stack, heap, and image boundaries when the runtime template
permits them.

Keep linker arguments and the runner on the final binary's exact target. A
library cannot reliably inject final-artifact layout for every consumer.

## Target contract

Record these facts:

| Fact | Evidence |
|---|---|
| Exact chip and revision | Schematic, board manifest, or chip marking |
| Rust target triple | Workspace config and `rustc --print target-list` |
| Runtime crate | Dependency graph and the binary entry attribute |
| Flash and RAM regions | Vendor memory map and bootloader reservation |
| Clock source and frequency | Clock tree configuration and measurement |
| Debug transport | SWD, JTAG, USB, serial, or semihosting |
| Reset and update path | Boot ROM, bootloader, watchdog, or debugger |

## Built-in and custom targets

A Tier 2 target in `rustup target list` ships a prebuilt `core` on stable. Rust
1.98 promoted `thumbv7a-none-eabi`, `thumbv7a-none-eabihf`,
`thumbv7r-none-eabi`, `thumbv7r-none-eabihf`, and `thumbv8r-none-eabihf` from
Tier 3 to Tier 2.

Since Rust 1.95 a custom JSON target spec needs a pinned nightly
([release post](https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/)): rustc
needs `-Z unstable-options`, Cargo needs `-Z json-target-spec`, and `core` needs
`-Z build-std=core`. Add one only when no built-in target fits. Start from
`rustc +<nightly> -Z unstable-options --print target-spec-json --target <nearest-builtin>`.

## Make `memory.x` visible to the linker

Follow the runtime or HAL layout-generation path when it has one. Otherwise,
make `memory.x` visible to the linker from every workspace directory. Copy it to
`OUT_DIR` in the firmware crate's `build.rs` and add that directory to the
linker search path. The `cargo::` directive syntax needs Cargo 1.77 or later;
use `cargo:` for an older MSRV.

```rust
use std::{env, fs, path::PathBuf};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    fs::copy("memory.x", output.join("memory.x")).expect("copy memory.x to OUT_DIR");
    println!("cargo::rustc-link-search={}", output.display());
    println!("cargo::rerun-if-changed=memory.x");
}
```

Do not add this build script when the selected runtime or HAL already emits the
layout and linker search path.

## Memory file

A memory file must use values from the selected chip and boot layout:

```text
MEMORY
{
  FLASH : ORIGIN = <application-flash-origin>, LENGTH = <application-flash-length>
  RAM   : ORIGIN = <usable-ram-origin>,         LENGTH = <usable-ram-length>
}
```

## Linker-script invariants

When you own the linker script, keep these invariants:

- Place the vector table at the address expected by the boot path.
- Retain vectors and required startup sections with `KEEP`.
- Copy initialized data from flash to RAM before Rust code reads it.
- Zero BSS before Rust code reads it.
- Align the stack, heap, DMA buffers, and vector table as the architecture needs.
- Reserve bootloader mailboxes, retained crash data, and peripheral DMA memory.
- Fail the link when sections overlap or exceed a region.
- Export only symbols that startup code or a verified diagnostic reads.

## Runner and link arguments

Put the runner and the link arguments on the final binary's exact target:

```toml
[target.<target>]
runner = "probe-rs run --chip <chip>"
rustflags = [
  "-C", "link-arg=-Tlink.x",
  "-C", "link-arg=-Tdefmt.x",  # only when the binary uses defmt
  "-C", "link-arg=--nmagic",   # when a memory.x origin is not 0x10000-aligned
]
```

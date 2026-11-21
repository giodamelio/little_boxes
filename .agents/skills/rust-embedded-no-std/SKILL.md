---
description: Use when writing, porting, or debugging bare-metal Rust firmware or no_std libraries, such as embedded-hal drivers, Embassy or RTIC tasks, flashing and logging with probe-rs and defmt, memory.x and linker scripts, panic and allocator policy, interrupt-shared state, and atomics on cores without compare-and-swap. Triggers on embedded Rust, cortex-m-rt, thumbv6m, critical-section, portable-atomic, panic_handler, or HardFault.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-embedded-no-std
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 7ab82bfd69cd09755798570cb6d2a19e6f2a72a6
name: rust-embedded-no-std
---
# Rust Embedded and no_std

Keep firmware bootable, bounded, observable, and host-testable. Add no executor,
allocator, or hardware abstraction until the product needs it.

Put a `// SAFETY:` comment above every `unsafe` block (the `rust-unsafe` skill owns
raw-pointer soundness, and `memory-model` atomic orderings with `Ordering::Release`/`Acquire`).
The `rust-native-linking` skill owns C and vendor SDK linking, and `rust-wasm` owns
`wasm32v1-none`.

## Failure triage

| Symptom | Likely cause | Check and fix |
|---|---|---|
| E0463 ``can't find crate for `std` `` | A dependency with default features, or a crate root without `#![no_std]` | `cargo tree --target <triple> -e features -i <crate>`; set `default-features = false` or add `#![no_std]` |
| `` `#[panic_handler]` function required, but not found `` | No panic provider in the binary | Link one, such as `panic-halt` or `panic-probe`, or define one `#[panic_handler]` |
| E0152 ``found duplicate lang item `panic_impl` `` | Two panic providers | Inspect `cargo tree`; keep one in the binary |
| `no global memory allocator found` | `alloc` with no allocator | Remove allocation or initialize one bounded heap |
| `` custom targets are unstable and require `-Zunstable-options` ``, or `` `.json` target specs require -Zjson-target-spec `` | JSON target spec on stable (Rust 1.95+) | Use a built-in target, or pin nightly (board-bring-up reference) |
| E0599 ``no method named `fetch_add` found`` (or `compare_exchange`, `swap`), or E0432 `` unresolved import `core::sync::atomic::AtomicU32` `` | Target has no CAS, or no atomic types | Use `portable-atomic`; the final binary selects its backend |
| ``invalid definition of the runtime `memset` symbol`` (or `memcpy`, `memmove`, `memcmp`, `bcmp`, `strlen`), deny-by-default since 1.98 | Wrong arity, a non-`extern "C"` ABI, or a `static` | Match the C signature, or remove `#[unsafe(no_mangle)]` or `export_name` |
| warning ``suspicious definition of the runtime `memset` symbol`` | Right arity, wrong parameter type (`c: u8` for `c_int`); caller and callee disagree on the ABI | Match the C types (`core::ffi::c_int`, `usize` for `size_t`); add `#![deny(suspicious_runtime_symbol_definitions)]` |
| `` `-C soft-float`: this option has been removed ``, or ``target feature `<name>` must be enabled to ensure that the ABI ...`` | Float ABI changed by a flag | Select the `eabi` or `eabihf` target that matches the linked C objects |
| `creating a shared reference to mutable static` (or `mutable reference`) | `static mut` under edition 2024 | Use an atomic, `critical_section::Mutex`, or framework resources |
| Linker reports region overflow | Code, statics, heap, or stack exceed layout | Inspect sections; reduce use or change only a verified memory budget |
| defmt link errors, or logs missing or corrupt | See the defmt reference | Follow its triage table |

Read [references/runtime-faults.md](references/runtime-faults.md) when firmware
builds but does not start, faults, hangs, or works only in a debug build. Do not
weaken optimization, add memory, or add delays as the fix before you find the
violated invariant.

Apply these rules to every change. Each one prevents a silent, unsound, or
irreversible defect:

- Access memory-mapped registers only through the PAC or HAL, or with
  `core::ptr::read_volatile` and `write_volatile`: the compiler can elide a plain dereference.
- Run no Rust code before RAM initialization. Write `__pre_init` in assembly
  (`global_asm!`), not `#[pre_init]`.
- Use `critical-section-single-core` (`cortex-m`) or `unsafe-assume-single-core` only on a
  single-core chip in privileged mode.
- Keep a DMA buffer alive, at a stable address, and untouched until the driver confirms the
  stop: a timeout, `select`, or dropped future does not stop DMA.
- Keep one memory-layout owner: add no `memory.x` copy in `build.rs` when the runtime or HAL
  has one.
- A successful flash does not prove that the image fits: the programmer can erase adjacent
  data first.

## Verify with the check that proves the claim

Keep parsing, state machines, conversions, and retry decisions independent of registers, and
test that library on the host triple (`cargo test --locked -p <logic-crate> --lib --target
"$host_target"`). Fake a peripheral only at the existing HAL boundary.

- Host tests prove logic only. A clean target build proves that the image links inside the
  declared regions, not the run-time stack or heap.
- A global `build.target` in a reusable workspace makes `cargo test` execute firmware on the
  host; pass the host target as above.
- Check each chip or runtime feature set; reject invalid combinations with `compile_error!`.
- A library that claims cores without CAS checks `thumbv6m-none-eabi` with
  `--features portable-atomic/critical-section` on the command line only, never in its manifest.
- Give `probe-rs` an explicit `--chip`; `probe-rs attach` preserves a running fault.
- Add hardware-in-the-loop checks only for what a host cannot prove, each with a timeout and a
  recovery action.

Read [references/verification.md](references/verification.md) when you write these checks.

## Completion checklist

For every change: reusable logic compiles as `no_std` and its tests run on the host triple; the
final binary owns one panic policy, one allocator if needed, one critical-section
implementation, and one defmt logger if it logs; interrupt, DMA, and task ownership are explicit
and bounded. For a no_std library, host tests and a target build are the evidence.

For a firmware release, or a change to boot, memory layout, interrupts, DMA, or panic policy,
also: record the chip, target, runtime, boot offset, and memory regions; give each budget
evidence; inspect, flash, and run the release ELF without a debugger, and record its
`DEFMT_LOG`; exercise the panic, watchdog, reset, queue-full, and transport-failure paths; and
measure calibration values on real hardware. Report each hardware item with no board or probe as
not run.

## Keep concurrency bounded

Use interrupt handlers only to acknowledge hardware and move bounded state. Document the access
contract of every interrupt-shared value (the reference lists its four facts).

- Do not share state through `static mut`: `&raw mut` still compiles and keeps the race. Use an atomic, a `critical_section::Mutex<RefCell<T>>`, or framework resources.
- Read CAS support from `rustc --target <target> --print cfg | grep 'target_has_atomic='`; with no
  line for the width, use `portable-atomic`, whose backend only the final binary selects.
- The final binary selects exactly one `critical-section` implementation (the HAL's on a
  multi-core chip).
- Never hold a critical section across an `.await`, a hardware wait, logging, unknown
  callbacks, or an input-dependent loop.
- Disabling local interrupts is not a cross-core lock.

Read [references/interrupts-and-atomics.md](references/interrupts-and-atomics.md) when you share
state with an interrupt or choose an atomic strategy. Treat DMA as concurrent access by hardware:
read [references/dma.md](references/dma.md) when firmware starts, completes, or cancels a DMA
transfer.

## Put no_std at the correct boundary

Make reusable logic a `#![no_std]` library; keep the runtime, startup, panic policy, and
hardware singleton in the final binary. A complete `lib.rs`:

```rust,ignore
#![no_std]
#![forbid(unsafe_code)]

pub fn saturating_scale(sample: u16, gain: u16) -> u16 {
    sample.saturating_mul(gain)
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::saturating_scale;

    #[test]
    fn scale_saturates() {
        assert_eq!(saturating_scale(u16::MAX, 2), u16::MAX);
    }
}
```

Stay in `core`; prefer a bounded container or caller-owned storage to `alloc`. When allocation is
required, record the heap facts of the resource-budgets reference, and install exactly one
`#[global_allocator]`, in the final program, never in a library. Do not retry an
allocation failure in an unbounded loop.

Set `panic = "abort"` in each shipped profile unless the target and runtime have a tested unwind:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
overflow-checks = true
debug = 2 # debug info stays in the ELF; it does not use flash
```

Provide exactly one panic implementation in the final dependency graph: a debug crate that
reports through the transport, or a production one that resets or records a bounded fault.
Libraries must not select the panic policy. A panic message is delivered only when the same
release artifact reports it on the real transport.

## Select the target and own startup

Use a built-in target when it matches the CPU, ABI, floating-point mode, and atomic support;
`rustc --target <target> --print cfg` is the source of truth. Since Rust 1.95 a custom JSON
target spec needs a pinned nightly. Do not infer the memory map from a similar board. Prefer the
runtime crate's linker template with the chip regions in `memory.x`, and keep linker arguments and
the runner on the final binary's exact target.

Read [references/board-bring-up.md](references/board-bring-up.md) when you inspect a workspace,
bring up a board or chip, need a custom target spec or `memory.x`, own a linker script, or
configure the runner.

## Select the hardware and task model

Use `embedded-hal` 1.0 traits at reusable driver boundaries; keep board pins, clocks, and
concrete HAL types in the application. A driver for a device with its own chip
select takes `SpiDevice`, not `SpiBus` plus an `OutputPin`, which breaks on a shared bus. Serial
I/O uses `embedded-io`; async traits live in `embedded-hal-async`. 0.2 paths such as
`digital::v2` do not exist in 1.0.

Select one task model:

| Requirement | Select |
|---|---|
| Short sequential control loop | Synchronous functions and interrupts |
| Many I/O waits, static async tasks | Embassy |
| Priority-driven tasks, static resources | RTIC |
| Vendor certification or scheduler contract | The required RTOS |

Do not mix Embassy and RTIC in one binary without a documented interrupt-ownership contract.

Read [references/task-frameworks.md](references/task-frameworks.md) when you
write, port, or review Embassy or RTIC code, including code copied from
pre-0.10 Embassy examples.

## Budget every finite resource

Give each release an explicit budget, with evidence, for flash, static RAM, stack, heap, queues,
interrupt latency, CPU time, and energy. Make a stack overflow fault instead of corrupting
statics with `flip-link` or `set-msplim`. Read
[references/resource-budgets.md](references/resource-budgets.md) when you set or review the
budget, set up the overflow guard, or set a calibration value such as a watchdog window.

Reject unbounded queues, recursive retry, and growth from input-controlled lengths. Treat a full
queue as a normal state with a deliberate policy: drop-oldest, drop-newest, backpressure,
coalescing, or fail-safe.

## Log from the device

Use `defmt` for bounded device logs, decoded with the exact ELF of the same build. Do not log
secrets or unbounded input. Read [references/defmt.md](references/defmt.md) when you change a
logger, its transport, or the log level (defmt emits only `ERROR` by default), or a log fails.

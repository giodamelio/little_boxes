# Interrupt-shared state and atomics

Read this when code shares state with an interrupt handler, chooses between native atomics and
`portable-atomic`, or selects a `critical-section` implementation.

Contents:

- Interrupt-shared state
- Atomic support by target
- Critical sections

## Interrupt-shared state


Use interrupt handlers only to acknowledge hardware and move bounded state.
Move parsing, formatting, allocation, and blocking I/O to task context.

For every interrupt-shared value, write down:

1. Which contexts read and write it.
2. Which operation makes access exclusive.
3. The maximum time that interrupts stay masked.
4. Which memory ordering or critical-section rule publishes the data.

Do not share state through `static mut`. Edition 2024 denies every reference to
it (`static_mut_refs`). `&raw mut` and by-value access still compile, but they
keep the data race with the interrupt handler. Use an atomic, a
`critical_section::Mutex<RefCell<T>>`, or the framework's resource model.

## Atomic support by target

Read atomic support from `rustc --target <target> --print cfg | grep 'target_has_atomic='`.
`target_has_atomic="32"` means that 32-bit CAS exists. A target that prints no
`target_has_atomic="N"` line has no CAS. `target_has_atomic_primitive_alignment`
lines describe alignment, not atomic support. Stable `--print cfg` does not show
load and store support. On `thumbv6m-none-eabi` and
`riscv32imc-unknown-none-elf`, `AtomicU32::load` compiles and `fetch_add` fails
with E0599. On `armv4t-none-eabi`, `armv5te-none-eabi`, and `msp430-none-elf`,
the import of `core::sync::atomic::AtomicU32` fails with E0432. Do not assume
native atomic loads; compile a one-line probe on the target.

Use native atomics only when `--print cfg` lists the required width. On a
target without CAS, use `portable-atomic`, which also supplies the missing
atomic types. A library depends on it without a backend feature, and enables
`require-cas` when it cannot work without CAS. The final binary selects the
backend: `critical-section`, or `unsafe-assume-single-core`.

## Critical sections

Let the final binary select exactly one `critical-section` implementation. On a
multi-core chip such as the RP2040, use the HAL's implementation. Do not create
a second interrupt-mask implementation in a driver.

Never hold a critical section across these operations:

- An `.await` point.
- A peripheral transaction that waits for hardware.
- Logging or formatting.
- A callback into unknown code.
- A loop with an input-dependent bound.

Model interrupt priority and preemption. Masking same-priority work is not proof
against a higher-priority interrupt. On a multi-core target, disabling local
interrupts is not a cross-core lock.

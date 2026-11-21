# Release resource budget

Use this table when you set or review the release budget. Each resource needs
the evidence in the second column.

| Resource | Required evidence |
|---|---|
| Flash | Linked section sizes plus update or bootloader reserve |
| Static RAM | Data, BSS, retained memory, and static task storage |
| Stack | Per-context estimate plus measured high-water mark |
| Heap | Configured size and measured high-water mark, or zero |
| Queues | Fixed capacity and full-queue behavior |
| Interrupt latency | Longest masked interval under release optimization |
| CPU time | Worst observed task and interrupt time with margin |
| Energy | Duty cycle under the real clock and peripheral configuration |

## Stack on Cortex-M

With `cortex-m-rt`, enable its `paint-stack` feature, run the worst-case load,
and read the stack high-water mark. The `flip-link` and `set-msplim` setup
below makes an overflow fault.

## Calibration

Clock tolerance, oscillator startup, watchdog window, debounce time, ADC offset,
sensor threshold, and radio timing vary by board and environment. Keep each one
as configuration. Set a safe default, limit the accepted values, and record the
real-hardware measurements that justify them.

## Heap facts

When allocation is required, define all of these facts:

- The heap address and size in the linker layout.
- The allocator implementation and initialization point.
- The earliest time that allocation is permitted.
- The out-of-memory behavior.
- The measured high-water mark under worst-case load.

An allocation failure is normally terminal on bare metal.

## Stack overflow guard

Run `cargo install flip-link --locked`, then set the linker of the final binary's target in
`.cargo/config.toml` (LLD only):

```toml
[target.<target>]
linker = "flip-link"
```

On Armv8-M Mainline (`thumbv8m.main-none-eabi*`), the `cortex-m-rt` `set-msplim` feature is the
alternative.

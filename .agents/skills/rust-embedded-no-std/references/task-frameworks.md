# Embassy and RTIC rules

Use these rules after `SKILL.md` selects Embassy or RTIC as the task model.

## Rules for both frameworks

- Bound every channel capacity. Define the overflow policy at the sender.
- Reject an interrupt vector that a HAL, executor, dispatcher, or monotonic owns
  twice.
- Verify timer progress and every asynchronous interrupt wake-up on the real
  device. A host test cannot prove either one.

## Embassy

Prove that every spawned task has a static slot. Prove that no task waits
synchronously inside the executor: a blocking wait stops every task on that
executor.

When Embassy uses time, select exactly one compatible time driver in the final
binary. Use its fixed tick rate, or select a rate only when the driver supports
that choice. Verify that the clock and interrupt-latency budget support the
rate. Bind each interrupt that an asynchronous driver requires through the
documented HAL or runtime mechanism.

Copy Cargo features and the spawn pattern from the upstream examples of the
release tag that you use, not from `main` or an older post. Code from an older
release does not compile against a newer executor. As of embassy-executor
0.10.0 (released 2026-03,
[changelog](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/CHANGELOG.md)):

- The `arch-*` features are named `platform-*` (0.10).
- A task call returns a `Result` that fails when the task pool is full (0.10).
  Unwrap it before the spawn: `spawner.spawn(blink(led).unwrap())`, or
  `spawner.spawn(defmt::unwrap!(blink(led)))` when the binary uses defmt.
  `spawner.spawn` returns `()`. Before 0.10, `spawner.spawn` itself returned
  the `Result`.
- Task pools are static on stable since 0.8. The `task-arena-size-*` features
  no longer exist. Remove them from an old `Cargo.toml`.

## RTIC

Prove resource ceilings, priority ordering, and the maximum lock duration.
Reserve each declared dispatcher for software tasks. Bind and start each
selected monotonic.

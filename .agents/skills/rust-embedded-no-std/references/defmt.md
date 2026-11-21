# defmt logger, transport, and log level

Use these rules when you add or change a `defmt` logger, its transport, or the
shipped log level.

## Logger and transport

Select exactly one global `defmt` logger and transport implementation in the
final binary. For RTT, link the workspace's compatible `defmt-rtt` crate or its
equivalent explicitly; `probe-rs run` decodes the stream but does not add the
transport to the firmware. Keep the logger-retention import in the binary, not
in a reusable library. Pass `-C link-arg=-Tdefmt.x` for the binary target.

Inspect the final dependency graph and link output for duplicate or missing
logger and panic symbols. Then run the release ELF through the real transport.

## Log level

defmt emits only `ERROR` messages unless `DEFMT_LOG` enables more
([filtering](https://defmt.ferrous-systems.com/filtering)). `DEFMT_LOG` is a
compile-time input. Set it in the build command or in the `[env]` table of
`.cargo/config.toml`, and record it with the ELF. `defmt::println!` ignores the
filter.

Prove the shipped level:

1. Compare `cargo size` of the release ELF built with and without the intended
   `DEFMT_LOG`.
2. Observe one line at that level on the real transport.

Measure flash, timing, and transport backpressure at the release level. A debug
probe that drains logs can hide a deadlock or timing failure that appears when
the device runs alone.

## Triage

| Symptom | Likely cause | Check and fix |
|---|---|---|
| Link errors that name defmt symbols or sections | No `-Tdefmt.x`, or zero or two global loggers | Add the link argument; link exactly one logger in the binary |
| Logs missing below `ERROR` | `DEFMT_LOG` not set for this build | Set `DEFMT_LOG` and rebuild; record it with the ELF |
| Logs corrupt or disappear | Wrong ELF, transport overflow, or reset during output | Match build identity and reduce bounded log volume |

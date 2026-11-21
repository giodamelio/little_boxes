# Runtime fault triage

Use this table when the firmware builds and flashes but misbehaves on the
target. Build and link errors are in the triage table of `SKILL.md`.

| Symptom | Likely cause | Check and fix |
|---|---|---|
| Firmware flashes but does not start | Wrong vector address, entry point, or reset setup | Check boot offset, vector contents, and runtime feature |
| Hard fault after enabling DMA | Buffer lifetime, alignment, cache, or region error | Stop DMA; inspect ownership and chip memory rules |
| Interrupt fires once | Pending flag is not cleared correctly | Read the peripheral sequence; clear only documented flags |
| Interrupt storm | Level source remains asserted or priority is wrong | Inspect source state and acknowledgement order |
| Async task stops progressing | Blocking call, lost wake, or exhausted task slot | Inspect executor trace and all bounded slots |
| Debug build works, release fails | Timing race, overflow, layout, or logging side effect | Test the release ELF and inspect optimization-sensitive sharing |

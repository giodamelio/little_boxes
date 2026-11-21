# DMA ownership, ordering, and cancellation

Use these rules when firmware starts, completes, or cancels a DMA transfer, and
when you review a DMA driver.

## Ownership

Treat DMA as concurrent access by hardware. Keep its buffer alive and at a
stable address until completion. Transfer ownership to the driver when its API
supports that model. Apply the chip's cache clean and invalidate rules. The
`rust-unsafe` skill, when it is installed, has the raw-pointer and
SAFETY-comment rules for a raw DMA buffer API. Without it, put a `// SAFETY:`
comment above every `unsafe` block that names the invariant the transfer needs.

## Ordering

Order buffer access against the transfer
([embedonomicon](https://docs.rust-embedded.org/embedonomicon/dma.html)):

- Put `fence(Ordering::Release)` after the last buffer write and before the
  register write that starts DMA.
- Put `fence(Ordering::Acquire)` after the completion check and before the first
  buffer read.

On a single-core Cortex-M0 to M4F with no data cache, which does not reorder
memory transactions, `compiler_fence` with the same orderings is enough. On
every other target, use `fence`, which emits a hardware barrier. This includes a
Cortex-M7, a core with a data cache, a multi-core chip such as the RP2040, and
RISC-V. Apply the chip's cache maintenance together with the fence.

## Cancellation

Treat DMA cancellation as a separate state transition. A timeout, a `select`
branch, or a dropped future does not prove that the transfer stopped. Do not
reuse or drop the buffer until the driver confirms that DMA stopped, clears the
pending interrupt, and completes the required cache maintenance. If the driver
cannot prove that state, keep ownership quarantined and reset the peripheral
through its documented recovery path.

# Termination and signals

Read this when you implement or change Ctrl-C, `SIGTERM`, or `SIGHUP` handling.

## Choose the handler

The standard library has no signal API. Use the facility the workspace already
has. Add a signal crate only when an interrupt can leave work half done:

| Situation | Facility |
|---|---|
| The binary runs a tokio runtime | `tokio::signal::ctrl_c`, and `tokio::signal::unix::signal` for `SIGTERM` and `SIGHUP` |
| No async runtime, Unix and Windows | The `ctrlc` crate, with a handler that sets an `AtomicBool` |
| No async runtime, Unix signals other than `SIGINT` | The `signal-hook` crate, registering an `AtomicBool` flag |

The work loop reads the flag between units of work and starts the shutdown
sequence below. A signal callback must do the minimum work its API allows. Do
not allocate, lock application mutexes, or write diagnostics from a raw Unix
signal handler.

Without a handler, `SIGINT` and `SIGTERM` end the process at once. No
destructor runs, so a `Drop` guard does not remove its temporary file and a
`BufWriter` does not flush. Measured on Rust 1.99.1, Linux: a process killed by
`SIGINT` exits by signal 2 and its `Drop` impl does not run. Give each temporary
file a recognizable name, such as `.<dest>.<random>.tmp` in the destination
directory, so a later run or the user can find and remove a leftover.

A handler replaces the default action. As of tokio 1.54.1, once `ctrl_c` or
`signal` registers a Unix signal, tokio captures every later delivery of it for
the life of the process, even after the future drops. A second Ctrl-C then does
nothing unless the code handles it: implement step 7 below explicitly.

## Signals per platform

On Unix, consider `SIGINT` and `SIGTERM`. Review `SIGHUP` separately because a
daemon and an interactive CLI can assign it different meaning. On Windows,
handle the console Ctrl-C event through the selected cross-platform facility.
Do not assume that every Unix signal exists on Windows.

## Shutdown sequence

Use this sequence:

1. The first termination request stops admission of new work.
2. Notify owned tasks through the existing cancellation mechanism.
3. Stop progress rendering and keep stderr available for the final diagnostic.
4. Let the current atomic output either complete or discard its temporary file.
5. Drain owned work for one finite grace period.
6. Return the documented cancellation status if shutdown completes.
7. On a second interrupt or grace expiry, use the product's forced-exit policy.

## Durability after a crash

State which file and directory sync operations the supported filesystem needs,
and test the failure states that the platform can reproduce. The sync steps of
the commit transaction are in [`atomic-output.md`](atomic-output.md).

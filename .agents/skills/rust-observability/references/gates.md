# Gates: why they are written this way, the repository gate, and lints

Deep material for `rust-observability`. SKILL.md holds the gate commands. Read this before you
change a gate line, when you write the repository gate, or when you tune the stdio and stack-frame
lints.

Contents:

- How the `cargo tree` gates fail closed
- The repository gate
- The stdio lint configuration
- `clippy::large_stack_frames`

## How the `cargo tree` gates fail closed

Run both checks once for each engine crate and inner FFI crate. Gate on the
`grep`, not on the exit status of an inverted `cargo tree -i`: it exits 101 when
the library is clean and 0 when it prints a real failure. Do not write `! grep`:
`set -e` ignores a negated command. Do not pipe `cargo tree` into `grep -q`
either: an early match kills Cargo with SIGPIPE, and a failed Cargo only makes
the `if` false. Each assignment fails only when Cargo itself fails, so under
`set -e` a wrong `-p` name stops the gate instead of passing it.

## The repository gate

Add a repository gate that reads the field tables from the observability crate
rather than repeating them, so the gate and the runtime visitor cannot disagree.
The gate rejects:

- a `?` or `%` sigil in an emission,
- `format!` inside an emission,
- `#[instrument]` without `skip_all`, or with `err` or `ret` on a path to an
  embedded sink,
- a field name that is not declared in the vocabulary,
- a crate whose declared coverage state disagrees with the coverage record.

Give the gate a `--self-test` mode that runs one known-bad fixture per rule,
and run it first; without it the gate rots into a no-op.

## The stdio lint configuration

```toml
# clippy.toml
allow-print-in-tests = true
```

```rust
// crate root: lib.rs or main.rs
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]
```

Read [redaction-and-field-vocabulary.md](redaction-and-field-vocabulary.md#exempt-a-terminal-tool-from-the-stdio-lints)
when a host-side tool must print to a terminal.

## `clippy::large_stack_frames`

The lint is in `nursery`, so it is off by default. Enable it and set `stack-size-threshold` in
`clippy.toml`. A span or an event macro can push a frame past a small threshold such as 4096
bytes. For a once-per-operation call, write
`#[expect(clippy::large_stack_frames, reason = "...")]`. In a hot loop, move the emission into an
`#[inline(never)]` helper, or delete it.

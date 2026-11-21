# WebAssembly size and optimizer control

Use this file when you set or defend a size budget, change the release profile,
or change how `wasm-opt` runs.

## Measure the shipped file

Measure the packaged file that consumers load, not the file in `target/`:

```bash
wc -c pkg/*_bg.wasm
gzip -9 -c pkg/*_bg.wasm | wc -c
twiggy top pkg/*_bg.wasm
wasm-tools objdump pkg/*_bg.wasm
```

The `twiggy` repository is archived. If it cannot parse the module, read the
section sizes from `wasm-tools objdump`.

Set a size budget on the packaged file. Report the absolute size and the change
from the baseline. Do not count generated JavaScript, TypeScript, or other
shipped assets as zero.

## Start from a measured release profile

```toml
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = "debuginfo"
```

Do not use `strip = "symbols"` on a WebAssembly target. With Rust 1.99.1 on
`wasm32-unknown-unknown` it removes the `name`, `producers`, and
`target_features` custom sections; `wasm-tools objdump` lists all three with
`"debuginfo"` and none with `"symbols"`. `wasm-opt` reads `target_features` to
learn which features the module uses, so without it the optimizer can reject
bulk-memory or other default instructions. If you must strip symbols, pass every
`--enable-*` flag of the next section to `wasm-opt`.

The WebAssembly targets already abort on panic, so the profile needs no `panic`
key. The release profile applies to every target in the workspace, so a native
release build gets the same settings.

Compare `opt-level = "s"`, `"z"`, and `3` on the final artifact. Smaller
codegen is not always a smaller compressed file or a faster application. `"s"`
and `"z"` also disable loop vectorization, which matters for SIMD-heavy code.

## Pin `wasm-opt`

`wasm-pack` runs any `wasm-opt` on `PATH` without a version check. If there is
none, it downloads the binaryen release that its own version pins:

| wasm-pack | Bundled binaryen |
| --- | --- |
| 0.13.1 to 0.15.0 | `version_117` |
| 0.15.1 (2026-10-30) | `version_130` |

The wasm-pack log line
`found wasm-opt at ...` shows when the `PATH` binary ran. Optimizer output and
supported proposals change between binaryen releases, and a wasm-pack upgrade
can change the bundled one. Pin the wasm-pack version, and pin `wasm-opt` in
one of two ways:

- Put a pinned `wasm-opt` on `PATH` in CI.
- Set `wasm-opt = false` under `[package.metadata.wasm-pack.profile.release]`
  and run a pinned binary as a separate step.

A `--panic-unwind` build needs a nightly toolchain with `-Zbuild-std`: the
rustup `std` for `wasm32-unknown-unknown` cannot unwind (Rust 1.99.1), so keep
it out of a stable product (`references/panic-and-errors.md`).
wasm-pack 0.15.1 adds `--enable-exception-handling` to the `wasm-opt` arguments
of a `--panic-unwind` build. wasm-pack 0.15.0 relied on the `target_features`
section instead, so its optimizer step failed when that section was stripped or
an older `wasm-opt` on `PATH` ran. If you run a pinned `wasm-opt` as a separate
step on a `--panic-unwind` build, pass `--enable-exception-handling` yourself.

Rust 1.87+ output uses bulk-memory and non-trapping float-to-int instructions
by default. If `wasm-opt` rejects them, enable them explicitly:

```toml
[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-Os", "--enable-bulk-memory", "--enable-nontrapping-float-to-int"]
```

This list replaces the default arguments, so keep an `-O` level in it. The flags
are a workaround from an open `wasm-pack` issue, reported while wasm-pack bundled
`version_117`. Check them against the pinned binaryen version: the `version_130`
bundle of wasm-pack 0.15.1 is not confirmed to need them.

After any optimizer change, run the full runtime test suite and the
engine-baseline validation from `SKILL.md` on the optimized file.

# Fuzzing and Differential Testing

Extended reference for the `rust-test-tools` skill. Read this when you set up cargo-fuzz, write
a fuzz target or a differential target, or turn a crash into a regression test.

## cargo-fuzz

proptest finds the bugs that a strategy can reach. A coverage-guided fuzzer also finds the
bugs that only a mutated corpus reaches, above all in binary protocol and container decoders.

```bash
# Setup, once per crate. Building and running a target needs nightly.
cargo install --locked cargo-fuzz
cargo fuzz init
cargo fuzz add parse_header
```

```rust
// fuzz/fuzz_targets/parse_header.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = my_crate::wire::parse_header(data);
});
```

```bash
cargo +nightly fuzz run parse_header -- -max_total_time=600
cargo fuzz list

# Reproduce a crash from a saved artifact, then shrink it.
cargo +nightly fuzz run parse_header fuzz/artifacts/parse_header/crash-<id>
cargo +nightly fuzz tmin parse_header fuzz/artifacts/parse_header/crash-<id>
```

- Fuzz the outermost entry point that takes untrusted bytes: a frame or header decoder, an
  archive reader, a decompressor, a document parser.
- A length field that drives an allocation is the usual out-of-memory crash. Cap the length in
  the parser, not in the fuzz target.
- A recursive parser needs a nesting-depth limit. Deep nesting overflows the stack. The
  report shows `AddressSanitizer: stack-overflow` in the default ASan build, or
  `has overflowed its stack` without ASan. Cap the depth in the parser, not in the fuzz target.
- Turn every crash into a regression test. Shrink it with `tmin`, commit the input (for example
  under `tests/regressions/`), and add a test that feeds it to the entry point.

## Differential testing

When a port's reference runs in-process (a C library, the previous Rust version, a spec
model), run a differential fuzz target; a Python or JavaScript original gets the `rust-port`
recorded comparison. Feed both the same bytes and compare. A port that is memory-safe and
passes its own tests can still disagree with the reference.

```rust
// fuzz/fuzz_targets/decode_matches_reference.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let ours = my_crate::decode(data).ok();
    let reference = reference_impl::decode(data).ok();
    assert_eq!(ours, reference, "decoders disagree on {data:?}");
});
```

- Compare acceptance and the value. Compare the error kind too when callers depend on it.
- Seed `fuzz/corpus/<target>/` with non-ASCII text and invalid UTF-8: a lone continuation byte
  (`0x80`), an overlong encoding (`0xC0 0xAF`), and a truncated 4-byte sequence. Ports often
  differ on non-ASCII and invalid UTF-8 input.

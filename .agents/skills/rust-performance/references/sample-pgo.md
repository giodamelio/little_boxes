# Sample-based PGO on Rust 1.100+

Use sample PGO when a representative CPU sample profile is available and an
instrumented training build is too intrusive. `-Cprofile-sample-use` consumes
sample data; `-Cprofile-use` consumes instrumentation data.

## Capture a representative workload

Record `rustc -vV`, source commit, target, CPU baseline, features, input, and
flags. Keep the unstripped training binary and its debug information. Recollect
after a source, feature, target, or important workload change. Use the same
optimization and CPU settings in the baseline and profile-use builds.

```bash
RUSTFLAGS="-Cdebuginfo=2 -Cforce-frame-pointers=yes" \
  cargo build --locked --release --target aarch64-apple-darwin
samply record --save-only --main-thread-only --rate 1000 \
  --output samples.json.gz ./target/aarch64-apple-darwin/release/myapp real-input.bin
```

On macOS, keep the matching dSYM. On Linux, select a sampler that supports the
CPU and the permissions available on that host. A denied sampling operation is
a failed prerequisite, not a reason to invent sample counts.

## Convert actual samples

Use a converter for the sampler's format. `llvm-profgen` consumes supported perf
recordings, not samply's Firefox JSON. Rust's `llvm-tools` component includes
`llvm-profdata`, but not every standalone LLVM utility.

For one noinline function, a narrow converter can symbolize sampled leaf PCs
and emit LLVM's text sample format. Use the linker symbol name, not a demangled
display name. Count source lines relative to the function definition. Preserve
sample weights. An observed zero entry count stays zero; it is not a call count.
Reject unresolved workload frames rather than guessing their line numbers.

A converter for one noinline function is not a general production converter.
Inlined calls, discriminators, callsite counts, and context-sensitive profiles
need their own mapping. Select a converter that supports the cases in the binary.

```bash
rustup component add llvm-tools --toolchain 1.100.0
LLVM_BIN="$(rustc +1.100.0 --print sysroot)/lib/rustlib/$(rustc +1.100.0 --print host-tuple)/bin"
"$LLVM_BIN/llvm-profdata" merge --sample --extbinary samples.txt -o samples.prof
"$LLVM_BIN/llvm-profdata" show --sample --all-functions samples.prof
```

Require nonzero samples in the intended hot functions. Keep the raw capture,
binary identity, converter version, and converted profile. Store raw captures
privately when they contain source paths, arguments, or host details.

## Prove application before measuring

```bash
RUSTFLAGS="-Cdebuginfo=2 -Cforce-frame-pointers=yes -Cprofile-sample-use=$PWD/samples.prof" \
  cargo rustc --locked --release --target aarch64-apple-darwin --bin myapp -- --emit=llvm-ir
```

Inspect build diagnostics. For a known hot function in an acceptance fixture,
check its emitted LLVM IR for `!prof` and the corresponding `function_entry_count`
metadata. Also check that `llvm-profdata show --sample` reports nonzero body
samples for that function. This proves consumption, not speedup or full coverage.

## Compare the final binaries

Warm up both binaries. Run at least five paired trials and alternate their order.
Use identical input and compare outputs before accepting a timing. Record each
trial, median, spread, binary bytes, and whether debug information is separate.
Keep sampling outside timed runs. Variable host load can invalidate a small
difference; report uncertainty instead of a speedup claim.

Record the compiler, source identity, and workload with the measurement. Replaying
a sample profile on another architecture can test format compatibility; it does
not validate that profile for production optimization on the other architecture.

Sources: [rustc profile-sample-use](https://doc.rust-lang.org/1.100.0/rustc/codegen-options/index.html#profile-sample-use),
[LLVM sample format](https://clang.llvm.org/docs/UsersManual.html#sample-profile-text-format),
[llvm-profgen inputs](https://llvm.org/docs/CommandGuide/llvm-profgen.html).

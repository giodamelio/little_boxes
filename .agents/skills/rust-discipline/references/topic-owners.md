# Topics other skills own

Route these topics by skill name, when the skill is installed. A row for a skill outside this
skill's install bundle names the API or command that carries the rule, for use without it.

| Skill | Topic |
|-------|-------|
| `rust-hot-path` | Allocation, type size, hasher choice, and `SmallVec` sizing on a profiled hot path |
| `rust-performance` | Profiling with `cargo flamegraph`, benchmarks, and allocation measurement |
| `memory-model`; `loom` setup in `rust-test-tools` | Atomic orderings, the ordering comment that names the data it publishes, `update`/`try_update`, and when to write a `loom` test |
| `rust-async-internals` | Blocking work in async code (`spawn_blocking`), locks across `.await`, executors |
| `rust-unsafe` | `unsafe` review, `// SAFETY:` comments, and the unsafe lint gate |
| `rust-panic-safety` | Panics at an FFI boundary, `catch_unwind`, and abort profiles |
| `uniffi-boundary`, `rust-jni`, `ffi-error-progress-cancel` | FFI contracts and error mapping; a UniFFI error type derives `uniffi::Error` |
| `rust-callback-bounds` | Which bound accepts which closure (`Fn`, `FnMut`, `FnOnce`), and callable fields such as `Box<dyn Fn(T)>` |
| `rust-type-erasure` | Type-keyed stores and the `'static` bound of `Any` |
| `rust-event-loop-state` | Handler sets that each need `&mut` to one state object |
| `rust-variance` | Lifetime coercion and variance as a breaking change: `&'a mut T` is invariant in `T` |
| `rust-serde` | Types whose encoded form is a contract; human-written config takes `deny_unknown_fields` |
| `rust-code-style` | Module layout, case and crate naming, visibility, and formatting |
| `rust-error-handling` | Error-type design: `thiserror` enums, `anyhow::Context`, and the `Error::source` chain |
| `rust-crate-architecture` | Crate splits and dependency direction |
| `rust-lints` | Workspace lint tables and `clippy.toml` |
| `rust-security`, `rust-sanitizers-miri` | Advisory triage, `deny.toml`, Miri, and sanitizers |
| `rust-crate-release` | SemVer bump (`cargo semver-checks`) and publish decisions |

# Heap profiling on the host

Deep material for `rust-performance`, section 1. Read it before you add the `dhat` crate.

- Linux: `heaptrack ./target/profiling/myapp`, or `valgrind --tool=dhat ./target/profiling/myapp`.
- Any host, on stable: the `dhat` crate 0.3.3. Put both the `#[global_allocator] static ALLOC: dhat::Alloc = dhat::Alloc;` item and `let _profiler = dhat::Profiler::new_heap();`, the first statement of `main`, behind a `dhat-heap` feature, so neither reaches the shipped binary. A bare call or `let _ =` drops the profiler at once and records nothing. Run `cargo run --locked --profile profiling --features dhat-heap -- <args>`, then open `dhat-heap.json` in the [DHAT viewer](https://nnethercote.github.io/dh_view/dh_view.html).
- A `dhat::assert_eq!` allocation-count test pins a win. The `rust-hot-path` skill has the test and its one-test-per-file isolation rule.

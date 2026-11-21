# Golden Files

Extended reference for the `rust-test-tools` skill. Read this when you write a golden-file
helper, bless or review a fixture, or compare text, binary, or image output.

## Golden files

A golden test compares output with a committed file. Use it for output that another party
reads: serialized records, exported reports, generated code, protocol frames, rendered images.
Put the fixtures in `tests/golden/` of the crate that owns the output, and the test in
`tests/golden_contracts.rs`. Cargo builds each `.rs` file directly under `tests/` as a test
binary, and it skips a subdirectory that has no `main.rs`.

```rust
use std::path::Path;

/// Compares `actual` with `tests/golden/<name>`. `BLESS_GOLDENS=1` rewrites the file.
fn assert_golden(name: &str, actual: &str) {
    // env! resolves at compile time, so the path does not depend on the working directory.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(name);
    if std::env::var("BLESS_GOLDENS").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("missing golden {}: {error}; bless it with BLESS_GOLDENS=1", path.display())
    });
    assert_eq!(actual, expected, "golden mismatch for {}", path.display());
}
```

```bash
cargo nextest run --locked -p <crate> --test golden_contracts                  # compare
BLESS_GOLDENS=1 cargo nextest run --locked -p <crate> --test golden_contracts  # bless
git diff -- tests/golden/                                                      # review
```

- Never set the bless variable in CI. A missing fixture must fail the run, and the helper
  panics for that reason.
- Bless only an intended output change. An unexpected failure means the code changed
  behavior: find out why first. Read the fixture diff, and explain the contract change in the
  commit message. Never bless to turn a red run green.
- Make the output deterministic before you compare it. Replace timestamps, generated IDs, ports,
  temporary paths, and host names with placeholders. Iterate a `HashMap` or `HashSet` in sorted
  order, or use a `BTreeMap`: std hash iteration order changes from one process to the next.
- Do not build a golden file or an `insta::assert_debug_snapshot!` from `Debug` (`{:?}`)
  output. std does not keep that format stable. A probe over every `char` found that Rust 1.98.1
  escapes U+115F, U+1160, U+3164, and U+FFA0, which Rust 1.97.0 printed as is, and that Rust
  1.99.1 prints U+FF9E and U+FF9F as is, which Rust 1.98.1 escaped. Serialize with the real
  output format instead.
- Text: compare byte for byte. Add `**/tests/golden/** -text` to the root `.gitattributes` (or
  `tests/golden/** -text` to a `.gitattributes` in the crate directory), so that a checkout
  with `core.autocrlf` does not rewrite the line endings. A pattern with a slash is anchored
  to the directory of its `.gitattributes` file.
- Binary: compare the bytes from `std::fs::read`. For a PDF or an archive, remove the embedded
  timestamps first.
- Raster images: compare decoded pixels, exactly or with a stated per-channel tolerance. Do not
  compare encoded PNG bytes: another encoder version or setting gives other bytes for the same
  pixels.
- If the crate already uses `insta`, use its macros and `cargo insta review`. Do not add a
  second golden mechanism.

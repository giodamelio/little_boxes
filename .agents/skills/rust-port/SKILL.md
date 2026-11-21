---
description: Use when porting or rewriting a C, C++, Python, Go, JavaScript, or TypeScript module in Rust that must keep the original's output. Covers the behavior inventory, tests ported first, the traps that change output (floor division, rounding, string length, ordered maps, strict parsing), a recorded comparison with the original, and PORTING.md. Not for calling a C library from Rust (use rust-native-linking).
license: BSD-3-Clause
metadata:
    github-path: skills/rust-port
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: ab6ebfa23a6a4ec89075230908f67ade0f9d2d6f
name: rust-port
---
# Rust Port

A port succeeds when Rust callers get the same results as the original's callers, for the inputs
that the original receives. A line-by-line transliteration compiles, keeps every accident of the
source language, and adds the borrow checker on top. Port the behavior. Design the Rust shape
from that behavior.

## The procedure

1. **Inventory the behavior.** Read the original, its tests, and its callers. List each public
   function, its inputs, its outputs, and each way it fails. Add the quiet behavior, because a
   port goes wrong there: output order, tie breaks, empty input, whitespace and case handling,
   integer limits, rounding, the length of non-ASCII text, and what counts as an error.
2. **Port the tests first.** Write one Rust test for each original test, and give it a name that
   maps back to the original. Stub the API with `todo!()` first so that they compile; then they
   fail until the port exists. When the original has no tests, record its outputs on the
   inventory inputs first ([A recorded comparison](#a-recorded-comparison)), and make those the
   Rust tests: a test written from a guess about the original proves nothing.
3. **Design the Rust API from the inventory**, not from the original's shape. Drop what exists
   only because of the source language: out-parameters, `NULL` returns, error codes, global state,
   fixed buffers, classes that only group functions, and type checks at run time.
4. **Implement against the ported tests.** Keep the original algorithm unless it is wrong for the
   input it gets; record a change of algorithm in the notes.
5. **Compare with the original** on recorded inputs ([Verification](#verification)).
6. **Write `PORTING.md`** ([The porting notes](#the-porting-notes)).

If the original has a bug or an ambiguity, keep its behavior, record it, and let the owner
decide. A port is not the place for a silent fix.

## Verification

Run these when a Rust toolchain is present, and fix every finding before you report:

```bash
cargo generate-lockfile   # a new crate only: --locked fails when Cargo.lock is missing
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

Commit the new `Cargo.lock` with the port.

| Claim | Check | A pass does not prove |
| --- | --- | --- |
| The port matches the original's tests | Every ported test passes; the count equals the original's | Behavior that no original test covers |
| The port matches the original on real input | A recorded comparison (below) | Inputs outside the recorded corpus |
| The port matches a reference that a test can call in-process (a C library, an earlier Rust version) | The differential fuzz target of the `rust-test-tools` skill | Inputs that the fuzzer did not reach |
| Output order and formatting match | A byte comparison of whole outputs, not a parsed comparison | Locale or terminal effects of the original |

### A recorded comparison

Run the original once on a corpus, store each input with its output, and make a Rust test replay
them. This works for any source language, including one that the Rust test cannot call:

1. Collect 30 to 100 inputs: real ones from logs or fixtures, plus the edge cases of the
   inventory. Add non-ASCII text, empty input, the largest and smallest numbers, and negative
   numbers for every division or modulo.
2. Run the original on each input, and record the exact command and the original's version in
   `PORTING.md`.
3. Store each input and output as a golden file pair, and add one test that runs the Rust entry
   point on each input and compares bytes. The `rust-test-tools` skill owns the golden layout,
   the `-text` attribute that keeps line endings exact, and the scrubbing of volatile fields.
4. Regenerate an output file only by running the original again, never from the port: a blessed
   Rust output is no longer an oracle.

## Traps that change output

Each row is a difference that a test written from the Rust side does not catch.

| Symptom in the port | Cause | Fix |
| --- | --- | --- |
| A negative result is off by one against Python | Rust `/` and `%` truncate toward zero; Python `//` and `%` floor | A floor helper (`floor_div`, `floor_mod` in the reference probe). `div_euclid` matches Python only for a positive divisor |
| A `.5` value rounds the other way | `f64::round` rounds half away from zero; Python `round()` rounds half to even; JavaScript `Math.round` rounds half up | `round_ties_even` for Python (Rust 1.77+); for JavaScript, `ceil` at an exact negative half and `round` otherwise. `(x + 0.5).floor()` is wrong near 0.5 and above 2^52 |
| A count or a column differs on non-ASCII text | `len()` counts UTF-8 bytes in Rust and Go, code points in Python, UTF-16 units in JavaScript | `chars().count()` for Python, `encode_utf16().count()` for JavaScript; never slice a `&str` at a Python or JavaScript index |
| Output lines come in a different order | Python `dict` and JavaScript `Map` keep insertion order; `HashMap` does not | A `Vec` of pairs or `indexmap::IndexMap` when order is observable; `BTreeMap` only when the original sorts |
| Input that the original accepts is rejected | `str::parse` rejects surrounding whitespace and `_`; Python `int()` accepts both, C `strtol` skips leading space and stops at the first bad character | Trim, or write the original's rule out in one function and test it |
| A line count differs | `str::lines` splits on `\n` and `\r\n` only; Python `splitlines()` also splits on a lone `\r`, `\x0b`, `\x0c`, `\x1c`, `\x85`, `\u2028`, and more | Split on the original's separator set when the input can hold them |
| Fields appear or vanish | Python `split()` with no argument is `split_whitespace`; `split(' ')` keeps empty fields | Match the call, not the name |
| A debug build panics where the original wrapped | Go integers and C unsigned integers wrap; Rust panics on overflow in debug builds | `wrapping_*` where the original wraps by contract; `checked_*` where input can be large; `i128` or a big-integer crate for Python `int` |
| Equal items swap places | `sort_unstable*` is not stable; Python `sorted` and JavaScript `sort` (ES2019+) are | `sort_by_key` or `sort_by` |
| JavaScript sorts `[10, 9, 1]` as `[1, 10, 9]` | The default JavaScript sort compares strings | Port the comparison that the original really used, then record it as a preserved quirk |
| A large JavaScript integer loses digits | JavaScript numbers are `f64`, exact only up to 2^53 | `f64` when the original depends on it, `i64` when it was always meant as an integer; record which |
| A sort of floats does not compile, or NaN lands in a new place | `f64` is not `Ord` | `sort_by(f64::total_cmp)`, and record the NaN position |

Read [references/source-languages.md](references/source-languages.md) ("Trap probe") for an
executed probe of each Rust-side claim and the `floor_div` and `floor_mod` helpers.


Read the same reference when you map the constructs
of one source language: memory and error handling of C and C++, Python classes and exceptions, Go
interfaces and goroutines, and JavaScript `null`, `undefined`, and promises.

## Keep the port in proportion

- Port the inputs that the original receives. Where Rust's standard library differs from the
  source runtime only on exotic input (Unicode digits, non-ASCII whitespace, unbounded integers),
  record the difference in `PORTING.md`. Write a compatibility shim only when the owner says such
  input occurs.
- Do not grow the API. Port the original's public surface, plus the standard traits Rust callers
  expect: `Debug`, `Clone`, `PartialEq`, `Display` and `std::error::Error` on the error type,
  `FromStr` for parsed text, and `Default` where the original has defaults.
- Do not port the source runtime. Use `str::trim`, `str::parse`, and `char` methods; do not
  rewrite Python's `strip`, `int()`, or `repr()`, or C's locale-dependent `isspace`.
- Do not write `unsafe` or raw pointers unless the port must also expose or call a C ABI. When the
  original is a C library that stays in use, bind it instead of porting it: a `-sys` crate with
  `build.rs` and `bindgen` (the `rust-native-linking` skill owns this, when it is installed).

## The porting notes

Write `PORTING.md` next to the code. A reviewer checks it in a few minutes:

```text
# Porting notes: <original name and version or commit>

## API map
| Original | Rust |

## Test map
| Original test | Rust test |

## Deliberate differences
<each behavior that now differs, the input that shows it, and why>

## Preserved quirks
<odd behavior kept on purpose, with the test that pins it>

## Verification
<the commands run and their results; the recorded-comparison command and corpus size;
 the checks that were not run, and why>
```

A difference that is not in `PORTING.md` is a bug in the port.

## Related skills

When installed: `rust-tdd` (characterization tests, the red-green cycle), `rust-test-tools` (the
differential fuzz target, golden files), `rust-panic-safety` (a C ABI that no panic may cross),
`rust-native-linking` (binding a C library that stays: `build.rs`, `bindgen`, `-sys` crates),
`rust-serde` (byte-identical `serde_json` output), and `rust-cli` (a ported command line tool:
`clap`, exit codes, the stdout and stderr contract).

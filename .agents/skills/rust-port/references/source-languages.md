# Source languages

How the constructs of each source language map to Rust in a port. Read the section of the
language you port from. The traps that change output are in `SKILL.md`; this file holds the
structural mapping and an executed probe of the traps.

- [C and C++](#c-and-c)
- [Python](#python)
- [Go](#go)
- [JavaScript and TypeScript](#javascript-and-typescript)
- [Trap probe](#trap-probe)

## C and C++

| In the original | In Rust |
| --- | --- |
| `malloc`/`free`, `new`/`delete`, `create`/`destroy` pairs | Owned values (`String`, `Vec`, `Box`) that drop on their own; `Drop` for any other resource |
| `NULL` meaning "absent" | `Option<T>` |
| Error codes, `-1`, `errno`, `ERR_*` macros | `Result<T, E>` with an error enum: one variant for each code, with `Display` and `std::error::Error` |
| Out-parameters (`int *err_line`, `long *out`) | Return values: data in `Ok`, details in the error variant |
| Flag bitmasks (`FLAG_A \| FLAG_B`) | An options struct with named fields and `Default`, or the `bitflags` crate; by default not a bare `u32` |
| `char *` plus a length, `strcmp`, `strcasecmp` | `&str` or `&[u8]`, `==`, `eq_ignore_ascii_case` |
| A pointer and a count | A slice; an iterator for a walk by index |
| Fixed-size buffers and their truncation | Growable types. Keep the limit only when it is behavior, and test it |
| `#define` constants and function-like macros | `const`, enums, and functions |
| Global or static mutable state | A struct that the caller owns |
| `strtol`, `atoi`, `sscanf` | `str::parse` with the matching integer type; it rejects leading space. `strtol` and `atoi` both skip leading space and stop at the first bad character (`atoi("12abc")` is 12); `atoi` returns 0 when no digit is present and has undefined behavior on overflow, where `strtol` clamps and sets `ERANGE`. Port that rule on purpose or reject the input, record which, and make overflow an error |
| Signed overflow (undefined behavior in C) | `checked_*` and an error: the original had no defined result to keep |
| `char` arithmetic on bytes | `u8` methods (`is_ascii_digit`, `to_ascii_lowercase`); `char` only for Unicode scalar values |
| C++ inheritance used for code reuse | Composition and plain functions |
| C++ virtual interfaces | A trait; an enum when the set of implementations is closed |
| C++ exceptions | `Result`; a panic only for a broken invariant |
| C++ RAII classes | The same idea: a type with `Drop` |
| `std::map` (ordered) | `BTreeMap`; `std::unordered_map` maps to `HashMap` |

## Python

| In the original | In Rust |
| --- | --- |
| An exception hierarchy | One error enum (or one for each area) with a variant for each exception class; data attributes become variant fields |
| `None` as a value | `Option<T>` |
| A string from a closed set (`kind in ("build", "test")`) | An enum with `FromStr` and `Display` for the text form |
| A `dict` or a class used as a record | A struct with named fields |
| `dict` iteration (insertion order, guaranteed since Python 3.7) | A `Vec` of pairs or `indexmap::IndexMap` when the order is observable |
| Duck typing, `isinstance` | A trait, or an enum when the set of types is closed |
| Unbounded `int` | A sized integer chosen on purpose, `checked_*` where input can be large; `i128` or a big-integer crate when the original relies on the size |
| `sorted(..., key=...)` | `sort_by_key` or `sort_by`; both are stable, like Python's |
| `try` / `except` around a parse | `?` with a mapped error, or `match` on the `Result` |
| Default arguments | A builder or an options struct with `Default`; two functions for two common calls |
| Generators and `yield` | An `Iterator` implementation, or a `Vec` when the sequence is short |
| `with` blocks | A scope and `Drop`; `std::fs::File` closes on drop |
| `@dataclass(frozen=True)` | A struct with private fields and getters, or public fields with `Clone` |
| `f"{x:.2f}"`, `str(x)`, `repr(x)` | `format!("{x:.2}")`. `{}` matches `str` for integers and strings only: `str(1.0)` is `1.0` and `str(True)` is `True`, where `{}` prints `1` and `true`. `{:?}` is not `repr`; test float, bool, and quoted text against the original |

## Go

| In the original | In Rust |
| --- | --- |
| `(value, error)` returns | `Result<T, E>` |
| `nil` | `Option<T>`; a nil slice and an empty slice are both an empty `Vec` unless the original tells them apart |
| An interface | A trait |
| A struct with embedded structs | Composition with named fields |
| Goroutines and channels | Threads with `std::sync::mpsc`, or async tasks when the original serves many connections |
| `defer` | A scope and `Drop`, or a guard value |
| `panic` and `recover` | A panic only for a broken invariant; `recover` around a request becomes a `Result` |
| Map iteration (random order) | Any order is faithful; sort before output when the original sorts |
| `range` over a string (runes) | `char_indices()`; the index is a byte offset in both languages |
| Integer overflow (wraps silently) | `wrapping_*` where the original depends on it |
| A shift by the bit width or more | Go gives 0 for a left shift and an unsigned right shift, and the sign for a signed right shift (`int64(-8) >> 64` is `-1`; checked with Go 1.27). Port it as `x.checked_shl(n).unwrap_or(0)`, and for a signed right shift as `x.checked_shr(n).unwrap_or(if x < 0 { -1 } else { 0 })`: Rust panics in debug builds and masks the count in release builds |
| A `string` that holds invalid UTF-8 | `&[u8]` or `Vec<u8>`, and `utf8_chunks` where text is needed; a `&str` cannot hold it |

## JavaScript and TypeScript

| In the original | In Rust |
| --- | --- |
| `null` and `undefined` | `Option<T>`; keep two states only when the original treats them differently |
| Thrown errors | `Result<T, E>` |
| Plain objects used as records | Structs with named fields |
| `Map` (insertion order) | A `Vec` of pairs or `indexmap::IndexMap` |
| `number` | `f64` when the original does math on it; `i64` or `u32` when it was an integer, after a check that no value exceeds 2^53 |
| A TypeScript union of string literals | An enum |
| A TypeScript discriminated union | An enum with data |
| Promises and `async` functions | Synchronous code when the work is not I/O; `async` only when the original waits on I/O |
| `string.length`, `charAt`, `substring` | UTF-16 units: `encode_utf16()` for lengths; never slice a `&str` at a JavaScript index |
| `JSON.stringify` | `serde_json`, with two differences: it writes the `f64` 1.0 as `1.0` where JavaScript writes `1`, and its map key order is sorted unless `preserve_order` is on, where JavaScript puts integer-like keys first and then insertion order |
| Bitwise operators (`x \| 0`, `>>`) | They convert to a 32-bit integer first: port them on `i32`, and mask a shift count to 5 bits as JavaScript does |

## Trap probe

The Rust-side claims of the trap table in `SKILL.md`, executed (Rust 1.99.1). `floor_div` and
`floor_mod` are the floor helpers that the first row names:

```rust,run
/// Floor division as Python's `//` defines it. `i64::MIN / -1` overflows in Rust, where Python
/// returns 2^63: check that pair when the input can reach it.
fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && ((a < 0) != (b < 0)) { q - 1 } else { q }
}

/// The remainder that pairs with `floor_div`: it takes the sign of the divisor. It corrects
/// `a % b` instead of computing `a - b * q`, which overflows near the limits.
fn floor_mod(a: i64, b: i64) -> i64 {
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) { r + b } else { r }
}

fn main() {
    assert_eq!((-7 / 2, -7 % 2), (-3, -1));
    assert_eq!((floor_div(-7, 2), floor_mod(-7, 2)), (-4, 1));
    assert_eq!(((-7i64).div_euclid(2), 7i64.div_euclid(-2)), (-4, -3));
    assert_eq!((floor_div(7, -2), floor_mod(7, -2)), (-4, -1));
    assert_eq!((floor_div(i64::MIN, 3), floor_mod(i64::MIN, 3)), (-3074457345618258603, 1));
    assert!(i64::MIN.checked_div(-1).is_none());

    assert_eq!((2.5f64.round(), (-2.5f64).round()), (3.0, -3.0));
    assert_eq!((2.5f64.round_ties_even(), 3.5f64.round_ties_even()), (2.0, 4.0));
    // JavaScript Math.round: half toward positive infinity, exact for every f64.
    let js_round = |x: f64| if x.fract() == -0.5 { x.ceil() } else { x.round() };
    assert_eq!((js_round(-2.5), js_round(2.5), js_round(0.49999999999999994)), (-2.0, 3.0, 0.0));
    assert_eq!(js_round(4503599627370497.0), 4503599627370497.0);
    assert_eq!((0.49999999999999994f64 + 0.5).floor(), 1.0); // the polyfill fails here

    let s = "héllo😀";
    assert_eq!((s.len(), s.chars().count(), s.encode_utf16().count()), (10, 6, 7));

    assert!("  42 ".parse::<i32>().is_err());
    assert!("1_000".parse::<i32>().is_err());
    assert_eq!("+5".parse::<i32>(), Ok(5));

    assert_eq!("a\u{2028}b\x0cc".lines().count(), 1);
    assert_eq!(" a  b ".split_whitespace().collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(" a  b ".split(' ').collect::<Vec<_>>(), ["", "a", "", "b", ""]);

    assert_eq!(i32::MAX.wrapping_add(1), i32::MIN);
    assert_eq!(i32::MAX.checked_add(1), None);

    let mut v = [(1, 'b'), (0, 'x'), (1, 'a')];
    v.sort_by_key(|&(k, _)| k);
    assert_eq!(v, [(0, 'x'), (1, 'b'), (1, 'a')]);
    let mut f = [2.0, f64::NAN, 1.0];
    f.sort_by(f64::total_cmp);
    assert_eq!(&f[..2], &[1.0, 2.0]);
    assert!(f[2].is_nan());
}
```


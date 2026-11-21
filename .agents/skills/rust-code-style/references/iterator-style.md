# Iterator style

Deep material for [SKILL.md](../SKILL.md).

Contents:

- Keep side effects out of expressions
- Iterator combinators stay pure (`for_each`, `needless_for_each`, the benchmark exception)
- Choose the collector by error contract (`collect`, `partition`, `filter_map`)
- Never skip the errors of `BufRead::lines()` (a runnable probe)

## Keep side effects out of expressions

Do not mix a side effect and a pure expression in the same statement. Either compute a
value and return it, or declare a binding and then mutate it:

```rust
use std::collections::HashMap;

fn build(host: &str, port: u16, value: u64) -> HashMap<String, u64> {
    // Good. Pure computation.
    let key = format!("{host}:{port}");

    // Good. Mutation in its own statements.
    let mut map = HashMap::new();
    map.insert(key, value);
    map
}
```

Do not hide a side effect inside an expression:

```rust
use std::collections::HashMap;

fn compute_value() -> u64 {
    42
}

fn lookup(map: &mut HashMap<String, u64>, key: String) -> u64 {
    // Bad. A side effect hides inside an expression.
    let result = map.entry(key).or_insert_with(|| {
        log::info!("cache miss"); // hidden side effect
        compute_value()
    });
    *result
}
```

## Iterator combinators stay pure

A closure in a `.map()`, `.filter()`, or `.collect()` chain has no side effect. Use an
explicit `for` loop when you need mutation or logging:

```rust
// Good. A pure combinator chain.
let names: Vec<_> = items.iter().filter(|i| i.active).map(|i| &i.name).collect();

// Good. A for loop for side effects.
for item in &items {
    if item.active {
        registry.register(&item.name);
    }
}

// Bad. A side effect inside a combinator.
items.iter().for_each(|i| registry.register(&i.name));
```

A `for` loop covers every side-effecting use of `.for_each()`, and it puts the effect in
plain sight. The catalog default bans `Iterator::for_each` through the `disallowed-methods`
list in `clippy.toml`, so the rule holds without a reviewer. The `rust-lints` skill holds the
configuration.

`clippy::needless_for_each` (pedantic) is the built-in alternative for a team that does not
want the ban. It fires on `items.iter().for_each(..)`, but not on a longer chain such as
`items.iter().filter(..).for_each(..)` (clippy 1.98.1).

`for_each` can be faster than a `for` loop over an adapter such as `Chain`, because it uses
internal iteration. When a benchmark shows that gain, keep the call with
`#[expect(clippy::disallowed_methods, reason = "...")]` and name the benchmark in the reason.

## Choose the collector by error contract

`Result<T, E>` implements `IntoIterator`, and it yields one item for `Ok` and zero for
`Err`. A `flat_map` or a `flatten` over a `Result` therefore deletes every failure with no
trace. The output type is `Vec<i32>`, not a `Result`, so no lint and no type error reports
the loss. Pick the collector from the contract the caller needs:

```rust
use std::num::ParseIntError;

let raw = ["1", "duck", "2"];

// Bad. `Err` yields zero items, so the failure disappears with no diagnostic.
let dropped: Vec<i32> = raw.iter().flat_map(|v| v.parse::<i32>()).collect();
assert_eq!(dropped, vec![1, 2]);

// Stop at the first error. `collect` short circuits and returns that `Err`.
let strict: Result<Vec<i32>, ParseIntError> =
    raw.iter().map(|v| v.parse::<i32>()).collect();
assert!(strict.is_err());

// Keep both sides. `partition` leaves the `Result` wrapper on each half.
let (ok, bad): (Vec<_>, Vec<_>) =
    raw.iter().map(|v| v.parse::<i32>()).partition(Result::is_ok);
let ok: Vec<i32> = ok.into_iter().filter_map(Result::ok).collect();
let bad: Vec<ParseIntError> = bad.into_iter().filter_map(Result::err).collect();
assert_eq!(ok, vec![1, 2]);
assert_eq!(bad.len(), 1);
```

Unwrap the two halves with `filter_map(Result::ok)` and `filter_map(Result::err)`. Each half
holds only one variant, so nothing is dropped. `map(Result::unwrap)` and
`map(Result::unwrap_err)` produce the same values and add a panic path.

Drop the failures only when the drop is the intent, and write that drop as
`filter_map` with `.ok()`. `filter_map` reads as a filter, so the reviewer sees the
deletion. `flat_map` reads as a mapping, so nobody does.

## Never skip the errors of `BufRead::lines()`

An intended drop is still wrong on a source that can repeat the same error. A reader that
fails on every call, such as a directory opened as a file on Unix, makes `lines()` yield `Err`
forever. `filter_map(Result::ok)`, `flat_map(Result::ok)`, and `flatten()` then never return
from the first `next()`. `clippy::lines_filter_map_ok` (warn by default) flags all three on
`io::Lines`. This probe proves the repeat, and shows the two collectors that stop:

```rust,run
use std::io::{self, BufRead, BufReader, Read};

/// A reader that fails on every call, like a directory opened as a file on Unix.
struct AlwaysFails;

impl Read for AlwaysFails {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read failed"))
    }
}

fn main() {
    // `lines()` yields `Err` again on every call, so it never returns `None`.
    let mut lines = BufReader::new(AlwaysFails).lines();
    for _ in 0..1_000 {
        assert!(matches!(lines.next(), Some(Err(_))));
    }

    // `map_while` stops at the first `Err`. `filter_map(Result::ok)` never returns here.
    let kept: Vec<String> = BufReader::new(AlwaysFails)
        .lines()
        .map_while(Result::ok)
        .collect();
    assert!(kept.is_empty());

    // Propagation keeps the error and also stops.
    let all: io::Result<Vec<String>> = BufReader::new(AlwaysFails).lines().collect();
    assert!(all.is_err());
}
```

Propagate the error when the caller must know that the read failed. Use
`map_while(Result::ok)` only when a silent stop at the first error is the intent. It also
stops on a transient error that a later call would recover from.

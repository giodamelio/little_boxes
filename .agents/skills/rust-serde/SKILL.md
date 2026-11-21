---
description: Use when deriving or changing Serialize or Deserialize on a type whose encoded form is a contract (config, stored record, or wire message), or when serde_json drops a field, reorders keys, or loses number precision without an error. Triggers on deny_unknown_fields, rename_all, serde(alias), serde(tag), untagged, serde(flatten), skip_serializing_if, serde(default), serde(try_from), DeserializeOwned, JSON map key, large integer, preserve_order, float_roundtrip, arbitrary_precision, postcard, "unknown field", "did not match any variant".
license: BSD-3-Clause
metadata:
    github-path: skills/rust-serde
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: e002640312c462562001fb3bad7e57947b2f589f
name: rust-serde
---
# Rust serde

A `#[derive(Deserialize)]` is a parser and a published schema at the same time. Most serde
incidents are silent: a field keeps its default, a number loses digits, or an older build cannot
read a payload.

Measured with serde 1.0.229, serde_json 1.0.154, postcard 1.1.3 (Rust 1.99.1), and bincode
2.0.1 (Rust 1.98.1).

## Find the contracts

Run these when you audit a crate or change a type that already shipped:

```bash
rg -n '#\[derive\([^)]*Deserialize' --type rust   # types whose encoding is a contract
rg -n '#\[serde\(' --type rust                     # every wire-format decision
# Files that derive Deserialize without deny_unknown_fields (`rg -L` means --follow).
rg -l0 'Deserialize' --type rust . | xargs -0 -r rg --files-without-match 'deny_unknown_fields'
# serde_json features in the shipped build, then in a test build with dev-dependencies.
cargo tree --locked -e normal,features -i serde_json -p <shipping-package>
cargo tree --locked -e features -i serde_json --workspace
```

## Decide what the encoding is for

| The encoded form is | Rule |
| --- | --- |
| Internal to one build, or a cache that may be discarded | Change it freely. Version the cache directory and drop it on mismatch |
| Written by a human: config, manifest, fixture | `deny_unknown_fields`. A typo must be an error, not a default |
| An older payload is read by a newer build of your own code | Self-describing format: additive only, and every new field gets `default`. postcard or bincode: version the whole payload |
| Read by another team or another language | Additive only, plus an explicit version field and a written schema |

Rows two and three conflict: `deny_unknown_fields` rejects a field a newer writer added. Apply it
to files a human authors, not to messages a newer peer may send.

## `deny_unknown_fields`

By default serde discards any key that matches no field. A typo in a config file passes, and the
field it was meant to set keeps its default. Nothing reports it.

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Config {
    host: String,
    timeout_secs: u64,
}
```

With the attribute, `{"host":"h","timoutSecs":5}` fails at parse time with
``unknown field `timoutSecs`, expected `host` or `timeoutSecs` at line 1 column 24``.

`rename_all` and `rename` are wire changes, not style changes: adding one to a shipped type
breaks every existing payload. Keep the old spelling readable with `alias`. Read
[references/schema-evolution.md](references/schema-evolution.md) when you rename, add, remove,
or retype a field of a shipped type.

## Enum representations

The external (default), internal (`tag`), adjacent (`tag` + `content`), and `untagged`
representations put four different documents on the wire. Choose once, when you create the type:
a later change is a breaking wire change.

- Internal tagging is the usual choice for a JSON message type. A newtype variant that holds a
  scalar compiles and fails only at runtime; use adjacent tagging for it.
- Use external tagging when the type must also go through postcard or bincode, or build without
  serde's `alloc` feature. The other three derive and then fail at runtime on those formats.
- Use `untagged` only for a small, shapeless input, such as a scalar or a list of scalars. Its
  error names no field, and the first variant that fits wins, even when a later one fits better.

Read [references/enum-representations.md](references/enum-representations.md) when you choose or
change a representation, or an enum fails at runtime in one format.

## `flatten`

`flatten` inlines a nested struct or a map into the parent, for example a
`#[serde(flatten)] extra: BTreeMap<String, serde_json::Value>` that captures unknown keys. Two
constraints that the attribute does not show:

- **`flatten` and `deny_unknown_fields` cancel each other.** The deny rule fires first, so the
  map never receives a key: `{"id":1,"typo":2}` returns ``unknown field `typo` ``. Serde
  documents the pair as unsupported. Pick the catch-all map or the strict schema.
- **`flatten` needs a self-describing format** (JSON, YAML, TOML). On postcard, serialization
  fails with `The length of a sequence must be known`.

## Choose a maintained format crate

Look a format crate up at `https://rustsec.org/packages/<crate>.html` before you add it. Before
commit, run `cargo deny --config deny.toml --locked check advisories` or
`cargo audit --deny warnings`. Plain `cargo audit` exits 0 on unmaintained and unsound
advisories. Never run a bare `cargo add bincode`: its latest release, 3.0.0, is a
`compile_error!` stub. Read [references/format-crates.md](references/format-crates.md) when you
add, keep, or replace bincode, serde_yml, or serde_yaml.

## Validate at the boundary with `try_from`

A deserialized value is untrusted input with a type. `#[serde(try_from = "..")]` runs a
conversion during deserialization, so an invalid value never becomes an instance:

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(try_from = "u16")]
pub struct NonZeroPort(u16);

impl TryFrom<u16> for NonZeroPort {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == 0 {
            return Err("port must be non-zero for this contract".to_owned());
        }
        Ok(NonZeroPort(value))
    }
}
```

`serde_json::from_str::<NonZeroPort>("0")` now fails. The error type must implement `Display`.
`#[serde(into = "..")]` does the same on the way out; it requires `Clone`.

## Stay readable across versions

Rules for an evolving type in a self-describing format (JSON, YAML, TOML):

1. Add fields; do not remove or retype them. A removed field breaks an older reader that requires
   it; a retyped field breaks every reader.
2. Give every added field `#[serde(default)]` in the same commit that adds it. Put
   `#[serde(default)]` on the struct as well when a section can be present but incomplete.
3. Rename with `alias`, not with a bare `rename`.
4. Carry an explicit version field when the format may need a real migration, and write the
   migration before you need it.

postcard and bincode decode by position: an added field fails on every old payload even with
`default`, and `skip_serializing_if` makes a type unable to read its own output. Version the whole
payload from the first release. The schema-evolution reference has the examples and errors.

## serde_json changes data without an error

| Operation | Default result | Control |
| --- | --- | --- |
| Parse an integer outside `i64`/`u64` into `Value` | Succeeds as `f64`: `18446744073709551616` becomes `1.8446744073709552e+19` | Parse into the typed field, or enable `arbitrary_precision` |
| `to_value` on an `i128`/`u128` outside the `i64`/`u64` range | `Err("number out of range")`; `json!` panics on it; `to_string` succeeds | Serialize directly, or enable `arbitrary_precision` |
| `f64` to JSON and back | Some values return one ulp off: `906.7979265841685` reads as `906.7979265841684` | Enable `float_roundtrip` (about 2x slower float parsing) |
| Key order of `Value` and `Map` | Sorted, because the map is a `BTreeMap` | `preserve_order` keeps insertion order; `Value::sort_all_objects()` (1.0.129+) sorts on demand |
| Key order of a derived struct | `to_string` writes declaration order; `to_value` then write sorts it | Serialize the struct directly |

Features unify across every package that one command builds, and a dev-dependency adds its
features only to test builds. One crate that enables `preserve_order` or `arbitrary_precision`
changes the result for every crate in the build, and a test can see a feature that the shipped
binary lacks. Decide the numeric and key-order policy in the crate that owns the wire format, pin
it with a test, and keep both features out of every dev-dependency. Compare the two `cargo tree`
commands from "Find the contracts".

Read [references/data-model-and-lifetimes.md](references/data-model-and-lifetimes.md) when a
generic parser needs a `Deserialize` bound, a map has non-string Rust keys, you must choose a
large-integer policy, or a test and the binary disagree on key order.

## Prove the wire contract

A derive proves only that serde can describe the Rust type. Test old-payload fixtures (assert the
field values, not only `is_ok()`), golden output, empty and `None` values, a round trip through
every claimed format, numeric edges, and the exact public map type. Read
[references/wire-tests.md](references/wire-tests.md) when you write these tests. Run the targeted
test while you iterate, then `cargo test --locked` once before commit. These tests do not prove
what another language's reader accepts: run the consumer's parser on the golden files.

## Triage

| Symptom | Cause | Fix |
| --- | --- | --- |
| A config value is silently the default | No `deny_unknown_fields`; the key was misspelled | Add the attribute; the parse now names the key |
| Every existing payload stopped parsing | `rename_all` or `rename` added to a shipped type | Add `alias` for the old spelling |
| `unknown field` on a key you meant to capture | `flatten` and `deny_unknown_fields` on one struct | Remove one of them |
| `did not match any variant of untagged enum`, or the wrong variant parses and drops fields | No shape fits, or an earlier variant with fewer fields fits first | Move to `tag` or `tag` + `content`; or put the variant with the most fields first |
| `cannot serialize tagged newtype variant ... containing ...`, or `invalid type: map, expected ...` on an enum | Internal tagging on a newtype variant that does not hold a map or struct | Use adjacent tagging |
| A newer build cannot read an older payload | A field was added without `default`; on postcard or bincode, a field was added at all | Add `default`. On postcard or bincode, version the whole payload |
| An older build cannot read a newer payload | The old reader rejects a new field | Emit the field only after tolerant readers are deployed |
| postcard or bincode fails at runtime on a type that JSON accepts | `flatten`; an internal, adjacent, or untagged enum; or `skip_serializing_if` | Use external tagging, no `flatten`, and no `skip_serializing_if`, or keep the format self-describing |
| ``implementation of `Deserialize` is not general enough`` | A generic parser requires `DeserializeOwned`, and the type borrows (`&'a str`) | Use `T: Deserialize<'de>` while the input lives; keep `DeserializeOwned` for owned input |
| `key must be a string` | A tuple, struct, or newtype-variant map key in JSON | Encode keys as strings, or use a sequence of key-value records |
| A large integer reads back as a float, or `number out of range` | The default `serde_json::Number` in `Value` | Parse into the typed field, or enable and test `arbitrary_precision` |
| An `f64` differs in the last digit, or key order changed after a dependency update | A `serde_json` default or a feature another crate enabled | See "serde_json changes data without an error" |
| A parsed value is structurally valid and semantically wrong | Validation lives after deserialization | `#[serde(try_from = "..")]` |

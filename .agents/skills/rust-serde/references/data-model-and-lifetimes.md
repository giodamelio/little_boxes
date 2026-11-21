# Serde Data Model and Lifetime Bounds

Use this reference when a type derives successfully but a generic parser, a map key, or a large
number fails. Behavior was measured with serde 1.0.229 and serde_json 1.0.154 on Rust 1.99.1.

Contents:

- Choose the deserialization bound from input ownership
- A derive does not prove format compatibility
- Define the large-integer policy
- Feature unification changes output between test and binary
- Authoritative references

## Choose the deserialization bound from input ownership

Use `Deserialize<'de>` when the result can borrow from the input:

```rust
use serde::Deserialize;

fn parse<'de, T>(input: &'de str) -> serde_json::Result<T>
where
    T: Deserialize<'de>,
{
    serde_json::from_str(input)
}
```

Use `DeserializeOwned` when the input buffer disappears before the result, or when the decoder
owns its input:

```rust
use serde::de::DeserializeOwned;

fn parse_owned<T>(input: String) -> serde_json::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str(&input)
}
```

A type that borrows (a `&'a str` field) cannot satisfy `DeserializeOwned`. The compiler reports
``implementation of `Deserialize` is not general enough``. Change the bound to
`Deserialize<'de>` and keep the input alive, or make the field owned (`String`).

Do not use `Deserialize<'static>` as an owned-data shortcut. It accepts only `'static` input, so
it rejects a normal `String` or read buffer. `DeserializeOwned` equals
`for<'de> Deserialize<'de>`, not `Deserialize<'static>`.

## A derive does not prove format compatibility

Serde defines a data model. Each format implements only the parts it can represent. A map key
is the common failure point.

JSON object keys are strings. `serde_json` writes these key types as strings: `String` and
`&str`, `char`, `bool`, integers up to `i128` and `u128`, floats, unit enum variants, and newtype
structs that wrap one of these. It rejects tuples, structs, sequences, maps, and newtype enum
variants with `key must be a string`.

Choose one contract:

- Convert keys to a canonical string and reject non-canonical spellings on input.
- Encode the map as a sequence of `{ "key": ..., "value": ... }` records.
- Select a format whose map keys support the required data model.

## Define the large-integer policy

The serde_json table in [SKILL.md](../SKILL.md) gives what `Value`, `to_value`, `json!`, and
`to_string` do with a large integer. This section adds the `Number` internals and the policy.

Without the `arbitrary_precision` feature, `serde_json::Number` holds an `i64`, a `u64`, or an
`f64`:

- `Number::from_i128` and `Number::from_u128` return `None` outside the `i64` and `u64` range.
- A typed `u64` field rejects a larger integer with
  ``invalid type: floating point `1.8446744073709552e+19`, expected u64``.
- The `to_string` path and the `to_value` path have different limits, so test the path that
  production uses.

`arbitrary_precision` stores the digits as text, so these values survive
`JSON -> Value -> JSON` exactly.

Do not convert an integer through `f64` unless the schema permits precision loss.
`9007199254740993` parsed as `f64` becomes `9007199254740992.0`.

Choose and document one policy:

| Contract | Required proof |
| --- | --- |
| Integer fits the default JSON number range | Reject outside the range before serialization |
| Integer is a decimal string | Validate the canonical string and its sign or width |
| Arbitrary-precision JSON number | Enable the feature deliberately and test every consumer |
| Binary integer | Select a format and schema with an explicit 128-bit representation |

## Feature unification changes output between test and binary

Normal-dependency features unify across every package that one command builds. A dev-dependency
turns a feature on only in builds that compile tests, examples, or benches. Measured: a
dev-dependency with `preserve_order` made `cargo test` print `{"b":1,"a":2}`, and the binary
printed `{"a":2,"b":1}`. The `cargo-workflows` skill, when it is installed, has the resolver
rules.

## Authoritative references

- [Serde deserializer lifetimes](https://serde.rs/lifetimes.html)
- [Serde data model](https://serde.rs/data-model.html)
- [Serde enum representations](https://serde.rs/enum-representations.html)
- [`serde_json::Number`](https://docs.rs/serde_json/latest/serde_json/struct.Number.html)
- [serde_json feature flags](https://github.com/serde-rs/json/blob/master/Cargo.toml)

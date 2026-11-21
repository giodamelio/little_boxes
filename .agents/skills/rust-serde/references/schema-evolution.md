# Schema evolution

Read this when you rename, add, remove, or retype a field of a type that already shipped, or
when a positional format (postcard, bincode) must read an older payload. Measured with serde
1.0.229, serde_json 1.0.154, and postcard 1.1.3 on Rust 1.99.1, and with bincode 2.0.1 on Rust
1.98.1.

Contents:

- `rename_all` is a wire change, not a style change
- Self-describing formats: `default` and `skip_serializing_if`
- postcard and bincode decode by position

## `rename_all` is a wire change, not a style change

`rename_all = "camelCase"` renames every field on the wire. A struct with that attribute and a
`timeout_secs` field rejects
`{"host":"h","timeout_secs":5}` with ``unknown field `timeout_secs` ``. Adding `rename_all` to a
type that already shipped breaks every existing payload.

Add `alias` for the old spelling when the type is already in the field:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    // Writes `timeout_ms`; reads both `timeout_ms` and the old `timeout`.
    #[serde(rename = "timeout_ms", alias = "timeout")]
    timeout_ms: u64,
}
```

`rename` sets the name for both directions. `alias` adds an accepted name for reading only, so
the new spelling is written and old payloads keep parsing. Keep the alias until every writer is
upgraded, then remove it in a release that says so.

## Self-describing formats: `default` and `skip_serializing_if`

Two attributes carry almost all backward compatibility. The rules in this section apply to
self-describing formats (JSON, YAML, TOML). postcard and bincode need the rules in the next
section.

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]     // a partially written section fills the rest from Default
pub struct Limits {
    pub max_bytes: u64,
    pub max_files: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub name: String,
    #[serde(default)] // the whole section may be absent in an older payload
    pub limits: Limits,
    // Omitted entirely when None, so an older reader never sees the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
```

Both `default`s are needed; they do different jobs. The field-level one handles an absent
section. The struct-level one handles a section that is present but incomplete:
`{"name":"a","limits":{"max_files":3}}` parses, and `max_bytes` comes from `Default`.

`skip_serializing_if` keeps a key out of the output when it has no value. A reader that predates
the field then sees no new key, which matters when that reader uses `deny_unknown_fields`.

## postcard and bincode decode by position

A positional format writes no field names, so the previous section does not hold:

- An added field fails on every old payload, even with `default` (postcard:
  `DeserializeUnexpectedEnd`).
- `skip_serializing_if` removes bytes that the reader expects, so the type cannot read its own
  output (postcard: `DeserializeBadOption`; bincode:
  `UnexpectedVariant { type_name: "Option<T>", .. }`). Do not use it on these formats.

Version the whole payload instead, from the first release: a version prefix, or an externally
tagged enum with one variant per version. A new variant goes at the end, and an old payload then
still decodes as its old variant.

# Enum representations

Read this when you choose or change the serde representation of an enum, or an enum fails
at runtime in one format. Measured with serde 1.0.229, serde_json 1.0.154, and postcard 1.1.3 on
Rust 1.99.1, and with bincode 2.0.1 on Rust 1.98.1.

Contents:

- The four representations
- `untagged` costs the error message and can pick the wrong variant

## The four representations

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum External { Ping, Data(u64) }

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Internal { Ping, Data { size: u64 } }

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
enum Adjacent { Ping, Data(u64) }

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Untagged { Number(u64), Text(String) }
```

For the `Data` variant carrying `3`:

| Representation | On the wire | Payload constraint | postcard, bincode (not self-describing) |
| --- | --- | --- | --- |
| External (default) | `{"data":3}` | None | Works |
| Internal (`tag`) | `{"type":"data","size":3}` | Struct, unit, or newtype holding a struct or map | Fails at runtime |
| Adjacent (`tag` + `content`) | `{"t":"data","c":3}` | None | Fails at runtime |
| Untagged | `3` | First variant whose shape fits wins | Fails at runtime |

Internal tagging is the usual choice for a JSON message type, because the tag reads naturally
and the payload stays flat. Adjacent tagging is the fallback when a variant holds something that
is not a map. Use external tagging when the type must also go through a binary format, or build
without serde's `alloc` feature. serde documents it as the only representation for no-alloc
builds, and the postcard and bincode results in the table show the same for those formats.

Internal tagging rejects a tuple variant at compile time. A newtype variant that holds a scalar
compiles and fails only at runtime:

```text
cannot serialize tagged newtype variant <Enum>::<Variant> containing an integer
invalid type: map, expected u64
```

The first line comes from serialization, the second from deserialization of `{"type":"data"}`.

On postcard, deserialization of the internal, adjacent, and untagged forms fails with
`This is a feature that PostCard will never implement`; bincode 2 reports `AnyNotSupported` or
`IdentifierNotSupported`. The derive compiles for all four.

## `untagged` costs the error message and can pick the wrong variant

An untagged enum tries each variant in order. When none fits, the error names no field and no
expected type:

```text
data did not match any variant of untagged enum Untagged
```

Nested in a struct, it adds only the position where the value ends (`at line 1 column N`). An
externally tagged enum at the same place names the expected type:
`invalid type: string "notnum", expected u64`.

The first variant that fits wins, even when a later one fits better. With variants
`Short { x: u8 }` and `Long { x: u8, y: u8 }` in that order, `{"x":1,"y":2}` parses as
`Short { x: 1 }` and drops `y` without an error. Put the variant with the most fields first, or
add `deny_unknown_fields` to the enum.

Do not use `untagged` for a message enum or a config section.

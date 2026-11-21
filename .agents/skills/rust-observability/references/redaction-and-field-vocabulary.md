# Redaction and the Field Vocabulary

This reference expands the redaction rules in `SKILL.md`. The gate and the
stdio deny list stay in `SKILL.md`; the exemption for a terminal tool is here. The dispatcher and sink registration are in
[dispatcher-install.md](dispatcher-install.md).

Contents:

- The vocabulary lives in exactly one place
- The three smuggling routes
- The redacting visitor
- What a sink receives
- Exempt a terminal tool from the stdio lints

## The vocabulary lives in exactly one place

Keep the normative allow list and deny list in one document — a privacy
document that covers every platform in the product, not a Rust document. Cite
it from code and from reviews. Never restate the list in a crate doc, a skill, a
gate script, or a commit message. Three copies drift, and the copy a reader
opens is then the wrong one.

Your observability crate holds the machine-readable tables that implement the
document. The gate script reads those tables. It does not repeat them. That way
the gate and the runtime visitor cannot disagree.

A workable table split:

| Table | Holds | Value shape |
|-------|-------|-------------|
| `ENUM_FIELDS` | Names whose value is a closed enumeration case | `&'static str` from a fixed set; record an integer code as its `&'static str` name |
| `COUNT_FIELDS` | Names whose value is a magnitude | Any integer type; a negative value is dropped |
| `FLAG_FIELDS` | Names whose value is a boolean | `bool` |

Nothing else is a legal field. A field with a free-text value has no table,
because there is no such field.

## The three smuggling routes

A field name that looks safe is not evidence that the value is safe. Watch for
these.

1. **A permitted name carrying a forbidden value.** A field called `count` that
   holds a fixed-point coordinate. A field called `code` that holds a hostname
   hash. The name passes the gate. The value is the leak.
2. **A correlation identifier derived from a forbidden value.** A stable hash of
   a location, a device identifier, or a user identifier is well-formed at every
   layer, and it confirms the value to anyone holding a candidate. A regex
   cannot see this. It stays reviewer-owned.
3. **An emission assembled in a helper rather than written at the call site.**
   The helper takes a `&str` and passes it into `tracing::info!`. The gate reads
   the macro call, sees a variable, and passes. The caller decides what the
   variable holds.

The gate cannot see route 2 or route 3. Both stay reviewer-owned. A green gate
is not a privacy proof. Say so in the review, every time.

## The redacting visitor

The visitor implements `tracing::field::Visit`. It keeps the declared fields and
drops everything else, including the message.

```rust
use tracing::field::{Field, Visit};

#[derive(Default)]
pub struct RedactingVisitor {
    fields: Vec<(&'static str, FieldValue)>,
    /// A declared name that arrived with the wrong value shape. Tests assert 0.
    dropped_declared: u32,
}

pub enum FieldValue {
    Enum(&'static str),
    Count(u64),
    Flag(bool),
}

fn is_declared(name: &str) -> bool {
    [ENUM_FIELDS, COUNT_FIELDS, FLAG_FIELDS].iter().any(|t| t.contains(&name))
}

impl Visit for RedactingVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if COUNT_FIELDS.contains(&field.name()) {
            self.fields.push((field.name(), FieldValue::Count(value)));
        } else {
            self.record_debug(field, &value);
        }
    }

    // i8, i16, i32, i64, and isize arrive here, not in record_u64.
    fn record_i64(&mut self, field: &Field, value: i64) {
        match u64::try_from(value) {
            Ok(v) => self.record_u64(field, v),
            Err(_) => self.record_debug(field, &value),
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if FLAG_FIELDS.contains(&field.name()) {
            self.fields.push((field.name(), FieldValue::Flag(value)));
        } else {
            self.record_debug(field, &value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match enum_case(field.name(), value) {
            Some(case) => self.fields.push((field.name(), FieldValue::Enum(case))),
            None => self.record_debug(field, &value),
        }
    }

    fn record_debug(&mut self, field: &Field, _value: &dyn core::fmt::Debug) {
        // Never keeps the value: `Debug` is an unbounded channel. The `message`
        // field arrives here and is dropped with everything else.
        if is_declared(field.name()) {
            self.dropped_declared += 1;
        }
    }
}
```

The important part is the shape, not the types.

- Every `record_*` method is an allow list. The default is to drop.
- Implement `record_i64`. tracing-core 0.1.36 sends `i8` to `i64` and `isize` to `record_i64`,
  `u8` to `u64` and `usize` to `record_u64`, and every unimplemented `record_*` method to
  `record_debug`. Without `record_i64`, a count passed as `i32` reaches `record_debug` and
  vanishes.
- `record_debug` never keeps a value. `tracing` routes the event message through the
  `message` field, so `record_debug` drops the message for free. It also drops every value
  recorded with `%`, `?`, `#[instrument(err)]`, or `ret`, because each one arrives through
  `Display` or `Debug`.
- A declared field that is dropped must be visible to a test. The visitor counts it in
  `dropped_declared`; assert 0 in the sink test for each emission, so a wrong value type fails
  the test instead of silently thinning the record.
- `record_str` maps a value to a known case. It does not copy the string. A
  string that is not a known case is dropped, not truncated.

## What a sink receives

```rust
pub struct RedactedEvent {
    pub level: Level,
    pub target: &'static str,
    pub callsite: &'static str,
    pub fields: Vec<(&'static str, FieldValue)>,
}
```

All four members are `'static` or plain data. Nothing borrows from the event.
There is no message, no error string, and no formatted line.

An error reaches the sink as its kind. Define the kind as a frozen enumeration
in the error type, and record it through an `ENUM_FIELDS` name.

```rust
tracing::error!(error_kind = err.kind().code(), "export failed");
```

The host formatter renders the whole line, message included. The embedded sink
receives `error_kind` and nothing else. Both are correct for their reader.

## Exempt a terminal tool from the stdio lints

Exempt only a host-side tool whose job is to write to a terminal. Put
`#![expect(clippy::print_stdout, reason = "...")]` after the `#![deny]` line, or
leave `print_stdout` out of that crate's deny list. An `expect` before the
`deny` loses: clippy reports the `println!` and an unfulfilled expectation.
`allow-print-in-tests` covers test code inside `src/`, where a path rule cannot
see it.

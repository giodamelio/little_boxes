# proptest property shapes

Deep reference for the proptest section of `SKILL.md`. `SKILL.md` holds the rules; this file
holds one example of each property shape, for proptest 1.12.0.

## Property shapes

```rust
use proptest::prelude::*;

proptest! {
    // Total function: no panic for any byte string.
    #[test]
    fn parse_never_panics(buf in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = Header::parse(&buf);
    }

    // Roundtrip: serialize a parsed header and get the consumed prefix back.
    #[test]
    fn parse_then_serialize_roundtrips(buf in prop::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(hdr) = Header::parse(&buf) {
            let mut out = Vec::new();
            hdr.write_to(&mut out);
            prop_assert_eq!(&out, &buf[..hdr.len()]);
        }
    }

    // Structured roundtrip: generate a valid value, encode it, and decode it back.
    #[test]
    fn decode_roundtrips_valid_input(value in arb_valid_message()) {
        let decoded = decode(&value.encode()).expect("valid input must decode");
        prop_assert_eq!(decoded, value);
    }

    // Text: "(?s).*" also generates '\n'. ".*" and any::<String>() never do.
    #[test]
    fn text_parser_never_panics(s in "(?s).*") {
        let _ = parse_document(&s);
    }
}
```

# Wire-contract tests

Read this when you write the tests that pin a serde type's wire contract: old-payload fixtures,
golden output, empty values, format round trips, numeric edges, and map keys.

Add tests for the risks the type carries:

- **Old payloads.** Commit one fixture per shipped version. Parse each with the current type and
  assert the field values, not only `is_ok()`. Include aliases, absent sections, and unknown keys.
- **Written output.** Compare the serialized form with a golden file, so a rename or a new
  `rename_all` shows up as a diff. The `rust-test-tools` skill, when it is installed, has the
  golden mechanics; without it, rewrite a fixture only in a run that sets `BLESS_GOLDENS=1`. When an older build must read the new output, parse the golden file with the
  released type. For a crate published to a registry, add a renamed dev-dependency on the same
  crate: `old = { package = "<crate>", version = "=<last>" }`.
- **Empty and absent values.** Round-trip empty strings, collections, maps, and `None` for every
  field that has `skip_serializing_if`. A skipped non-`Option` field without `default` writes a
  document that the same type cannot read, and serde_json reports `missing field` for it.
- **Every claimed format.** Round-trip each enum and each flattened type through every format the
  type claims, such as postcard. Tagged enums and `flatten` compile and fail only at runtime.
- **Numeric edges.** Test zero, signed minimum, unsigned maximum, the largest value inside the
  documented JSON range, and the first value above it. For `18446744073709551616`, assert
  `is_f64()` on the `Value` path, rejection on the typed `u64` path, or the exact digits under
  `arbitrary_precision`.
- **Map keys.** Serialize the exact public map type, not a `HashMap<String, T>` stand-in.

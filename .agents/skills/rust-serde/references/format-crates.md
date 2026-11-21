# Format crate status

Status as of 2026-09. Re-check each crate at `https://rustsec.org/packages/<crate>.html`
before you rely on it.

- `bincode` is unmaintained (RUSTSEC-2025-0141, which lists wincode, postcard, bitcode, and rkyv
  as alternatives). Its latest release, 3.0.0, is a `compile_error!` stub, so `cargo add bincode`
  gives a build that fails with only `error: https://xkcd.com/2347/`. Existing users keep a
  caret requirement, `bincode = "2.0.1"` or `bincode = "1.3.3"`, until they migrate. A caret
  requirement already excludes 3.0.0, so an `=` pin adds nothing. Never run a bare
  `cargo add bincode`; write `cargo add bincode@2.0.1`.
- `serde_yml` is unsound and unmaintained (RUSTSEC-2025-0068, which lists serde_norway and
  serde_yaml_ng as alternatives).
- `serde_yaml` is deprecated: its repository is archived, and its last release is
  `0.9.34+deprecated`.

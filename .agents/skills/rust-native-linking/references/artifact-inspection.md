# Inspect final artifacts

Read this when you inspect the architecture, dependency table, or exported
symbols of the exact shipped file. `references/windows.md` has the full
PE/COFF procedure.

## Commands

```bash
# Linux
file <artifact>
readelf -h -d --dyn-syms --wide <artifact>
nm -D --defined-only <shared-library>

# macOS
file <artifact>
lipo -info <artifact>
otool -L <artifact>
otool -l <artifact>
nm -gU <shared-library>
```

```text
rem Windows Developer Command Prompt
dumpbin /HEADERS <artifact>
dumpbin /DEPENDENTS <artifact>
dumpbin /IMPORTS <artifact>
dumpbin /EXPORTS <dll>
```

For a static archive, inspect its members and defined symbols with `ar t` and
`nm`. For a shared library, recurse through every dynamic dependency. A direct
dependency can load and still fail because one of its dependencies is absent.

## Symbol names

Rust symbols use v0 mangling (`_R` prefix) by default since Rust 1.97, so a
`_ZN` grep finds nothing. `#[no_mangle]` and `#[export_name]` symbols keep
their literal names. For a tool that predates v0, pipe its output through `rustfilt`.

## Exported symbols of a `cdylib`

Do not use `-Wl,--version-script` to hide exports of a Rust `cdylib`. rustc
passes its own version script, and an extra map did not remove a
`#[no_mangle]` export (lld, Rust 1.99.1). Compare the defined dynamic symbols
with an allowlist instead.

# Runtime loading on Linux and macOS

Use this file when you make a packaged program find its shared libraries at run
time: an ELF `RUNPATH`, a Mach-O install name and `LC_RPATH`, a Cargo-run smoke
test that passes while the package fails, or a macOS `library 'foo' not found`.

## Linux ELF

Prefer a relocatable `DT_RUNPATH` such as `$ORIGIN/../lib` for an application
that ships private libraries. Quote `$ORIGIN` in shell commands so the shell
does not expand it. Remember that `DT_RUNPATH` applies only to direct
dependencies. Give each shared object a valid path to its own dependencies.

Use `LD_LIBRARY_PATH` for diagnosis, not as the installed product contract.
`SKILL.md` has the `ldd` rule.

## Cargo-run smoke tests

`cargo::rustc-link-search` solves link-time discovery. It does not install a
shared library beside the final program. Cargo adds link-search paths under
`OUT_DIR` to the loader environment for commands such as `cargo run` and
`cargo test`. It does not guarantee this behavior for external system paths.
A Cargo-run smoke test can therefore pass while the packaged program fails.

For a published Rust package, also run `cargo package --list`, create the
`.crate`, extract it into a temporary directory, and build it there. This
proves that headers, native sources, and generated inputs enter the published
archive.

## macOS SDK root

On macOS since Rust 1.91, rustc passes the SDK root to the linker, so a
library in `/usr/local/lib` can fail with `ld: library 'foo' not found`. Find
the prefix with `pkg-config` and emit
`cargo::rustc-link-search=native=<prefix>/lib`.

## macOS Mach-O

Give a relocatable library an install name such as `@rpath/libfoo.dylib`.
Give the executable or loading library an `LC_RPATH` such as
`@loader_path/../Frameworks`. Inspect the result with `otool -L` and
`otool -l`. Use `install_name_tool` only as an explicit packaging step. Prefer
correct linker inputs so rebuilt artifacts do not need repair.

Embed and sign frameworks or dylibs in the final bundle. Check every required
architecture slice with `lipo -info`.

## Authoritative references

- [Linux dynamic loader](https://man7.org/linux/man-pages/man8/ld.so.8.html)
- [Apple run-path dependent libraries](https://developer.apple.com/library/archive/documentation/DeveloperTools/Conceptual/DynamicLibraries/100-Articles/RunpathDependentLibraries.html)

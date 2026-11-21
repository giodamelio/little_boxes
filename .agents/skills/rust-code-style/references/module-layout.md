# Module layout

Deep material for [SKILL.md](../SKILL.md).

Contents:

- Prefer `name.rs` plus `name/` (and `clippy::mod_module_files`)
- `lib.rs` is the table of contents (no glob re-export)
- A `prelude` module is the one sanctioned glob target (E0659 and E0034 downstream)
- Avoid `#[path]` (`include!` for `build.rs` output)

## Prefer `name.rs` plus `name/`

Use the `name.rs` file next to a `name/` directory for every module that has children:

```text
src/
  lib.rs
  session.rs          # module root, declares the sub-modules
  session/
    plan.rs
    report.rs
    tests.rs
```

The module root sits next to its children, and every editor tab carries a distinct name.

Use `mod.rs` only for a deeply nested submodule, three levels or more
(`session/runners/mod.rs`). There, a module that lives wholly inside its own directory moves or
deletes as one unit, which outweighs a distinct tab name. Do not mix both patterns at the same directory level: a reader
then cannot predict where a module root is.

To enforce `name.rs` everywhere, and to drop the deep `mod.rs` exception, enable
`clippy::mod_module_files` (restriction). It rejects every `mod.rs` file.

## `lib.rs` is the table of contents

`lib.rs` declares the modules at the top, then re-exports the public API item by item:

```rust
mod config;
mod session;
mod transport;
mod types;

pub use session::{SessionReport, run_session};
pub use types::{Request, Response};

#[cfg(test)]
mod tests;
```

The `pub use` lines follow the style edition 2024 sort order that `cargo fmt` applies to an
edition 2024 crate: `SessionReport` sorts before `run_session`.

Default: no glob re-export from `lib.rs` (`pub use types::*`). Two reasons:

- A reader cannot trace the API surface without opening every module.
- A glob exports every new item automatically, so a new `pub` item silently becomes part
  of the public API.

List every re-exported item explicitly.

## A `prelude` module is the one sanctioned glob target

A `pub mod prelude` beside `lib.rs` re-exports the few traits and types a caller needs in
every file. It is the one module callers are invited to glob-import. `lib.rs` itself stays
glob-free.

```rust
pub mod net {
    pub struct Socket;
    pub struct Config;
    pub mod prelude {
        pub use super::{Config, Socket};
    }
}

pub mod db {
    pub struct Config;
    pub mod prelude {
        pub use super::Config;
    }
}

mod consumer {
    use crate::db::prelude::Config; // an explicit import wins over the glob below
    use crate::net::prelude::*;

    pub fn open() -> (Config, Socket) {
        (Config, Socket)
    }
}
```

An addition to a prelude can break a downstream crate that glob-imports it, in two ways:

- A new name that also arrives through a second glob fails with E0659, `Config` is
  ambiguous, and the note `ambiguous because of multiple glob imports of a name in the same
  module`. The error points at the use site, not at the `use` line, so the caller reads it far
  from the import. One explicit import fixes it, as the `consumer` module above shows. A name
  the downstream defines itself also shadows the glob, and it compiles clean.
- A new trait with a method name that another in-scope trait also has for the same type fails
  every such call with E0034, `multiple applicable items in scope`, although no name collides.
  A fully qualified call (`Trait::method(&value)`) or explicit imports instead of the glob fix
  it.

Treat a new trait in a prelude as a possible break for glob importers (both measured on Rust
1.98.1).

Build the prelude itself from explicit `pub use` items, never from globs. When two globs in the
prelude export the same name, your crate compiles with only an `ambiguous glob re-exports`
warning. Every downstream crate that uses the name then fails: on Rust 1.98.1 with
"`Config` is ambiguous" from the deny-by-default `ambiguous_glob_imports` lint. A `-D warnings`
gate on your crate catches the warning before you publish.

Keep the prelude short. A prelude that mirrors the whole public API guarantees the
collision, and it hides which module owns each name.

## Avoid `#[path]`

Keep the standard module lookup rules. `#[path]` breaks the map between a module path and
a file path, so readers and tools cannot find the source.

For a file that `build.rs` generates, use `include!` instead:

```rust,ignore
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
```

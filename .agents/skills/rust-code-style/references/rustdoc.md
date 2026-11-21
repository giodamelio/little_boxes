# Rustdoc sections and doc examples

Deep material for [SKILL.md](../SKILL.md). Read it when you write a doc comment on a public item
or a doc example.

## A doc comment with `# Errors` and `# Panics`

```rust
use std::path::Path;

pub enum ConfigError { NotFound, PermissionDenied, InvalidUtf8 }

/// Reads a configuration file and parses it as UTF-8.
///
/// # Errors
///
/// - [`ConfigError::NotFound`] if `path` does not exist.
/// - [`ConfigError::PermissionDenied`] if the process cannot read it.
/// - [`ConfigError::InvalidUtf8`] if the bytes are not valid UTF-8.
///
/// # Panics
///
/// Never. Every failure is reported through the returned `Result`.
pub fn read_config(path: &Path) -> Result<String, ConfigError> {
    todo!()
}
```

## Doc examples are tests

Use `?` in an example by returning a `Result`, and hide the scaffolding with a
leading `#`:

```text
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = mycrate::read_config("app.toml".as_ref())?;
/// assert!(!config.is_empty());
/// # Ok(())
/// # }
/// ```
```

Rustdoc compiles a line that starts with `# ` but does not show it. Use `no_run` for an example
that must compile but must not run. SKILL.md has the rule for `ignore`.

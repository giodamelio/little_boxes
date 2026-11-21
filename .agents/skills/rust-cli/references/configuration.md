# Configuration sources and precedence

Deep material for [SKILL.md](../SKILL.md). Read it when you add a configuration source, a list
setting, or an environment variable. The trust rule for project configuration stays in SKILL.md.

## Precedence

Define one precedence order and use it for every setting. A common order is:

```text
built-in default < system config < user config < project config < environment < CLI
```

Use the order required by the product. Do not add every source automatically.
Merge typed partial values, not serialized documents. Preserve the difference
between absent, explicit `false`, explicit zero, and an empty list.

For each setting, define whether a higher source replaces or extends a list.
Replacement is the safe default. If lists extend, define the order and a way to
clear inherited values.

## Source rules

- Treat an explicit `--config PATH` that does not exist as an error.
- Ignore a missing auto-discovered optional file.
- Report a present file that cannot be read or parsed.
- Resolve relative paths inside a config file from that file's directory unless
  the documented format says they use the process working directory.
- Parse environment values with the same range and enum validation as CLI values.
- Report an invalid present environment value. Do not silently use a default.
- Provide explicit negative flags such as `--no-color` when a boolean can come
  from lower-precedence sources.
- Record source provenance internally so diagnostics can name the winning value.
- Do not print secret values in effective-configuration output.

Keep security-sensitive precedence (credentials, endpoints, proxies, TLS trust) separate when
ordinary setting precedence would let a project file cross the trust boundary in SKILL.md.

Read configuration before starting irreversible work. Validate the complete
effective configuration once. Do not let separate subsystems reinterpret the
same variable with different defaults.

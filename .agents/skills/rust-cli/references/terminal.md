# Terminal behavior

Read this when you add or change color, progress output, paging, prompts, or
`NO_COLOR` handling.

## Defaults

| Feature | Terminal | Non-terminal |
|---|---|---|
| Color | Auto when supported | Off |
| Progress | On stderr when useful | Off, or explicit stable log records |
| Pager | Only by explicit policy | Off |
| Prompt | Allowed when input and the prompt stream are terminals | Fail with an actionable flag |
| Table decoration | Human-readable | Keep the selected output format stable |

## Color precedence

Support `--color=auto|always|never` when color is part of the product. When
project policy adopts the `NO_COLOR` convention, a present and non-empty value
disables color in auto mode. User-level config and a per-invocation flag should
override `NO_COLOR` (no-color.org FAQ). `NO_COLOR` is a cross-program default,
so it ranks below the program's own config, unlike a program-specific
environment variable in the precedence chain. The `anstream` auto mode, which
`clap` uses for color, already applies `NO_COLOR`, `CLICOLOR`, and
`CLICOLOR_FORCE`; only an explicit `ColorChoice` overrides them.

A prompt written to stderr is not visible when stderr goes to a log, so a redirected stderr also
rules out a prompt.

## Controlling terminal

If the product opens the controlling terminal directly, make that a separate
explicit path and test it.

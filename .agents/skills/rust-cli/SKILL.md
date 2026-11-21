---
description: Use when designing, changing, or reviewing the user-visible contract of a Rust CLI, such as clap arguments and their compatibility, stdout and stderr separation, CLI exit codes, config precedence, TTY and color behavior, Ctrl-C and SIGTERM handling, broken pipe errors, atomic file replacement, non-UTF-8 path arguments, and packaged shell completions. Not for server graceful shutdown; use `rust-networking`.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-cli
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 5059961d1a83e8888fdd369f5e997ee0ec587cfc
name: rust-cli
---
# Rust CLI

Preserve the parser, configuration format, and runtime that the workspace already uses. Do not
add `clap`, an async runtime, a signal crate, or a temporary-file crate only to copy an example
from this skill.

## Define the contract

Decide each touched surface before you change code; a new command decides every row.

| Surface | Required decision |
|---|---|
| Arguments | Grammar, defaults, conflicts, required values, and `--` behavior |
| Compatibility | Which names, values, output fields, and exit codes scripts can depend on |
| Standard output | Data format, ordering, encoding, and broken-pipe result |
| Standard error | Diagnostics, warnings, progress, and quiet behavior |
| Configuration | Sources, precedence, merge rules, and path base |
| Termination | Interrupt behavior, drain deadline, and partial-output policy |
| File output | Overwrite policy, atomicity, durability, and metadata policy |
| Terminal mode | Color, progress, prompts, paging, and non-terminal fallback |
| Completions | Supported shells, generation point, and package destinations |

Documented behavior and behavior that automation uses are compatibility surface: before you
change an existing name, value, output, or exit status, inspect release notes, completions, man
pages, scripts, and tests, not only the parser. A changed default or stdout format, a newly
required value, a renamed, removed, or reinterpreted name or value, and a changed exit status
break scripts; restore the old behavior or release a documented breaking change. Read
[`references/compatibility.md`](references/compatibility.md) when you add or change a flag,
subcommand, positional, value, default, version line, exit status, or machine-readable format.

## Triage failures

| Symptom | Likely cause | Fix |
|---|---|---|
| `tool \| head` prints a panic | `println!` panics on `BrokenPipe` | Propagate writes and map stdout `BrokenPipe` to success |
| Last rows missing, status 0 | The drop-time flush failed and `BufWriter` ignored the error, or `process::exit` skipped the drop | Call `flush()` and return `ExitCode` from `main` |
| Error prints as `Error: Os { code: 2, kind: NotFound, .. }` with status 1 | `main` returns `Result` and std prints `Debug` | Print a `Display` diagnostic and return `ExitCode` |
| JSON parser sees a banner | Diagnostics use stdout | Move all non-data output to stderr |
| Redirected output contains escape codes | TTY state is not checked per stream | Use auto color from the destination stream |
| CI waits forever | A prompt runs with non-terminal stdin | Fail and require an explicit flag |
| CLI flag does not override config | Sources merge in subsystem order | Merge typed partials once in documented precedence |
| `--no-*` cannot undo config | Boolean absence and false collapse | Preserve explicit boolean provenance |
| Old output disappears on failure | Destination is truncated or removed first | Write beside it and replace only after success |
| Windows overwrite fails | Unix rename semantics are assumed | Use the selected Windows atomic replace facility |
| Ctrl-C leaves a truncated file | Signal exits inside the write | Cancel work and discard the temporary file |
| A valid path is rejected | Path is forced through UTF-8 | Keep it as `OsStr` or `Path` until a text boundary |

## Verify at the process boundary

Test through the built executable: take its path from `env!("CARGO_BIN_EXE_<name>")`, because a
hand-built `target/debug` path breaks under `--target`, `CARGO_TARGET_DIR`, or another profile.

With `clap`, add one unit test that calls `Cli::command().debug_assert()`; it proves nothing
under `cargo test --release`. Read
[`references/streams-and-exit.md`](references/streams-and-exit.md#the-clap-definition-test) for
where it must live.

Cover each touched contract surface with a process test. Read
[`references/process-tests.md`](references/process-tests.md) when you write or review them. Run them with `cargo test --locked -p <crate> --test <cli_test>` while you
iterate, and the workspace gate once before merge. Run them on each operating system whose
signal, path, and replacement behavior differs: a Unix-only green run does not prove Windows
replacement or console-event behavior.

Report the decision per touched surface, the process tests run per operating system, and any
unverified shell, platform, packaging, or crash-durability behavior.

## Separate stdout from stderr

Write requested data to stdout, and everything else (diagnostics, progress, banners, notices,
log records, prompts) to stderr.

Do not use `println!` for fallible streaming output: it panics when the write fails. Lock stdout,
propagate write errors, and call `flush()` on a `BufWriter` before it drops, because drop ignores
a flush error and the command then exits 0 without its last rows. Map stdout `BrokenPipe` to
success only when stdout is the only incomplete destination; a broken pipe on an upload, socket,
or required output file is a failure. Read
[`references/streams-and-exit.md`](references/streams-and-exit.md) for the behavior probe and
the `SIGPIPE` rules.

Escape machine output with the format encoder, never with string formatting. In a Unicode-only
format, give a non-UTF-8 path an explicit error or a documented lossless form, not silent
replacement characters.

`--quiet` suppresses informational stderr, never an error that explains a nonzero exit.
`--verbose` changes diagnostic detail, not stdout data. Redact secrets from both streams and
from debug error chains.

## Replace output safely

Do not truncate the destination before computation succeeds. Write a new temporary file in the
destination directory, then commit it with one atomic operation. Create it with
`OpenOptions::create_new(true)` under an unpredictable name, with the required mode, such as
`OpenOptionsExt::mode(0o600)`, in the same open call. Never use `File::create` on a predictable
temporary name: it follows a planted symbolic link and truncates its target.

`std::fs::rename` replaces an existing destination, so it is never a no-clobber commit, and an
existence check before it races with a concurrent creator. Keep atomic visibility and post-crash
durability as separate claims. Read [`references/atomic-output.md`](references/atomic-output.md)
when the command writes, replaces, or refuses to overwrite a file.

## Keep exit codes small and stable

Use a documented, portable set unless an established CLI already has one:

| Status | Meaning |
|---|---|
| `0` | Requested operation completed, or a stdout pipe closed normally |
| `1` | Operational failure |
| `2` | Invalid invocation or configuration |
| `130` | Graceful termination after an interactive Ctrl-C, when documented |

Add a code only when automation must distinguish a result, and keep it in `0..=125`. Never
expose raw `errno`, an HTTP status, or an OS error code as the exit code.

Return `std::process::ExitCode` from `main`. Never call `std::process::exit` while a buffer or
guard is alive: it runs no destructors. With clap, parse before you create one, because
`Cli::parse()` calls `std::process::exit`. Do not return `Result<(), E>` from `main` for a
user-facing error: std prints the `Debug` form. Read
[`references/streams-and-exit.md`](references/streams-and-exit.md#exit-codes) when you map a
clap error raised after work starts.

Print one primary diagnostic per failure, with the failed object and the next useful action;
keep detailed causes behind verbose mode and print no backtrace by default.

## Merge configuration with provenance

Define one precedence order and use it for every setting, for example
`built-in default < user config < project config < environment < CLI`. Merge typed partial
values, not serialized documents, so absent, explicit `false`, zero, and an empty list stay
distinct. Treat an explicit `--config PATH` that does not exist as an error, and report an
invalid present environment value instead of a silent default. Read
[`references/configuration.md`](references/configuration.md) when you add a configuration
source, a list setting, or an environment variable.

Treat an auto-discovered project configuration as untrusted until the user trusts that project or
selects the file explicitly. It must not replace a credential source, credential helper,
authenticated endpoint, proxy, or TLS trust policy from user configuration. Bind credentials to
the origin they authenticate, and do not send them after a project setting changes the endpoint.

## Design one unambiguous argument grammar

Keep one command definition as the source for parsing, help, and completions. Do not read a
password, token, or private key from an ordinary argument: process listings and shell history
show command lines. Do not use prefix matching for subcommands or enum values; with `clap`, keep
`Command::infer_subcommands` and `Command::infer_long_args` off.

Use `PathBuf` or `OsString` for paths and other operating-system values, and never call
`to_str().unwrap()` on them. Convert to UTF-8 only at a boundary that requires text, with an
error that names that boundary. Unix path bytes need not be UTF-8, and a Windows path can hold an
unpaired surrogate. [`references/compatibility.md`](references/compatibility.md) holds the other
grammar rules.

## Handle terminals without changing data semantics

Detect each stream separately with `std::io::IsTerminal` or the existing terminal library. Do not
add `atty`: it is unmaintained (RUSTSEC-2024-0375) and unsound on Windows (RUSTSEC-2021-0145).
Use TTY state only for presentation and interaction, never to change records, field types, or
exit meaning. Emit no control sequences to a redirected stream unless the user forces color with
`--color=always` or `CLICOLOR_FORCE`.

Never prompt in CI, a pipe, a service, or when stdin or the prompt stream is redirected; fail
with an actionable flag instead. Require an explicit confirmation flag for destructive
non-interactive work; `--yes` grants no authority the caller lacks. Read
[`references/terminal.md`](references/terminal.md) when you change color, progress, paging,
prompts, or `NO_COLOR` handling.

## Terminate gracefully

Treat Ctrl-C as a cancellation request, not as permission to abandon a write. An unhandled
`SIGINT` or `SIGTERM` ends the process without destructors, so a temporary-file guard never
removes its file: give each temporary file a recognizable name, such as `.<dest>.<random>.tmp`.
Before long-running work, install a handler that only sets a flag or wakes a task. Do not claim
cleanup for `SIGKILL`, power loss, abort, or forced Windows termination. Read
[`references/signals.md`](references/signals.md) when you implement or change Ctrl-C, `SIGTERM`,
or `SIGHUP` handling.

## Generate completions from the parser

Generate completions from the command definition of the released binary. With
`clap`, keep `clap_complete` on the same major version and use `clap_complete::aot::generate` or
`generate_to` (clap_complete 4.6). Completion generation must not make network requests, read
secrets, or read user configuration. Read [`references/completions.md`](references/completions.md)
when you add a completions command, package completions, add a shell, or change the release job.

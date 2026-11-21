# CLI process tests

Read this when you write or review the process-boundary tests of a CLI command.

## Coverage checklist

Cover the contracts that the change touches. A new command covers all of them:

- valid invocation, invalid invocation, help, and version statuses;
- exact stdout data and the absence of diagnostics on stdout;
- useful stderr and the documented operational status on failure;
- a consumer that closes stdout after the first byte while the tool still has
  more than 1 MiB to write: assert status 0 and no panic text on stderr. Linux
  pipe capacity is 16 pages, which is 1 MiB on a 64 KiB-page kernel. In bash,
  read `PIPESTATUS[0]` or use `set -o pipefail`, because `$?` is the status of
  `head`;
- stdin, stdout, and stderr redirected independently;
- non-UTF-8 path arguments when the command accepts paths. Build them with
  `OsStrExt::from_bytes` on Unix and with an unpaired surrogate through
  `OsStringExt::from_wide` on Windows. APFS on macOS rejects a non-UTF-8 file
  name with `EILSEQ` (os error 92), so do not create that file there;
- every configuration source alone and the complete precedence chain;
- an invalid explicit config file and invalid environment value;
- Ctrl-C during idle work and during output replacement;
- an injected write or sync failure that preserves the old destination;
- refusal to overwrite without the explicit overwrite option;
- terminal auto behavior with both terminal and pipe-backed streams;
- generated completion files for every supported shell.

## Terminal tests and goldens

Use a pseudo-terminal test only for behavior that depends on terminal state.
Do not mark a test interactive and then skip it in CI. Keep machine-format
goldens small and review changes as compatibility changes.

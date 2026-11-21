# Shell completion packaging

Read this when you add a completions command, package completions, add a
supported shell, or change the release job that produces completion files.

## Generation

Keep the completion companion crate on the parser's version and inside the
existing build or release feature policy. Do not package `clap_complete`
`CompleteEnv` output as a static file. As of clap_complete 4.6.9, it needs the
`unstable-dynamic` feature, and its shell code calls back into the binary
through an interface that can change on upgrade.

Provide a deterministic command such as:

```bash
tool completions bash > tool.bash
tool completions zsh > _tool
tool completions fish > tool.fish
tool completions powershell > tool.ps1
```

The command writes only the script to stdout. It writes no color, progress, or
banner. Reject an unsupported shell with the usage-error status.

## Package paths

Package files in the locations required by the target package manager. Common
system locations include `share/bash-completion/completions/<tool>`,
`share/zsh/site-functions/_<tool>`, and
`share/fish/vendor_completions.d/<tool>.fish`. Treat them as package-relative
paths, not universal installation paths. For PowerShell, follow the target
installer or module policy instead of editing a user's profile during install.

## Release and verification

Regenerate completions in the release job after the final binary metadata is
fixed. Compare generated files with tracked package assets when the repository
tracks them. Test that every supported shell parses or loads its script in a
clean environment. Run the applicable syntax checks on a host that has each
supported shell:

```bash
bash -n tool.bash
zsh -n _tool
fish -n tool.fish
pwsh -NoProfile -Command '$e=$null; [System.Management.Automation.Language.Parser]::ParseFile("tool.ps1", [ref]$null, [ref]$e) > $null; if ($e) { exit 1 }'
```

Also load each file in a clean shell and request a completion. A syntax check
does not prove that the generated function is registered.

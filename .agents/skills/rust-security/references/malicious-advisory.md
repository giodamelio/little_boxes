# Respond to a malicious advisory

Read this when an advisory has `categories = ["malicious"]`. Build scripts and proc macros
run at compile time, and rust-analyzer runs both when it loads the workspace. A runtime
payload can run under `cargo test`.

The `cargo audit` terminal report does not show the category. For a malicious release
cargo-audit 0.22.2 prints `Solution: No fixed upgrade is available!`, even when the advisory
lists newer `unaffected` versions. The unreleased rustsec `main` prints
`Upgrade to <range> (unaffected)` in that case. Do not match on the `Solution` line. Read the
category and the `unaffected` ranges with the `--json` filter in `references/incidents.md`.

1. Take the malicious code out of the graph in one change.
   - If the advisory lists `unaffected` versions, a legitimate crate had a compromised
     release. Roll back to the newest unaffected version:
     `cargo update -p <crate>@<bad-version> --precise <unaffected-version>`. If the
     `Cargo.toml` requirement excludes that version, lower the requirement first.
   - If it lists no `unaffected` versions, remove the dependency.
   - Confirm it. For a removed or injected package (such as `proc-macro1`),
     `grep -n 'name = "<package>"' Cargo.lock` prints nothing. For a rollback,
     `cargo tree --locked -i <crate>@<bad-version>` fails.
2. Delete the local copies of each malicious package, because the next build can run them
   again. Run `cargo clean` in every workspace that built the crate.

   ```bash
   reg="${CARGO_HOME:-$HOME/.cargo}/registry"
   find "$reg/cache" -name '<crate>-<version>.crate' -delete
   find "$reg/src" -mindepth 2 -maxdepth 2 -type d -name '<crate>-<version>' -exec rm -rf {} +
   cargo clean
   ```

   Delete the CI caches of each job that built it (`gh cache delete --all`, or a new
   key), and any sccache or `cargo vendor` copy.
3. Report to the owner which machines and CI runners built, tested, or opened the
   workspace. Recommend that they rotate every credential those machines could read:
   registry tokens, SSH keys, cloud credentials, and wallet keys. Rotation is an external
   action. Do not do it yourself.

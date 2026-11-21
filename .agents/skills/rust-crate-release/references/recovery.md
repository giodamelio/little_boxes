# Release Recovery and Owners

Read this when you consider a yank, a crate deletion, an owner change, or a RustSec advisory for
your own release. Every command here is an external write: get authorization for the exact
command first.

## Recover from a bad release

Do not use a yank as the normal fix. Publish a compatible fixed version first, when possible.
Check exact-version dependents and supported release lines before any yank.
Yank only for an exceptional defect: an accidental publish, an unintended SemVer break, or a
seriously broken version. Get authorization first.

```bash
cargo yank <package>@<version> --registry <registry>
cargo yank <package>@<version> --undo --registry <registry>
```

A yank blocks new resolutions; `cargo update --precise <version>` can still select it. It does
not break existing lockfiles, delete downloads, or remove a leaked secret.

crates.io lets an owner delete a whole crate only when it was published less than 72 hours ago,
or when it has one owner, fewer than 1000 downloads for each month, and no reverse dependencies
on crates.io. Deletion is an external write that needs authorization. It does not recall
downloaded copies.

If a secret entered the archive, revoke the secret first, then contact the registry. Do not wait
for a yank or a deletion.

For a vulnerability in your own release, publish the fixed release first. Then file a RustSec
advisory: a crate owner opens a pull request against `rustsec/advisory-db` with a file in
`crates/<package>/` ([RustSec contributing](https://rustsec.org/contributing.html)). Request a
CVE or a GitHub Security Advisory when the project wants one, and add its ID to `aliases`. Each
step is an external write that needs authorization.

## Manage owners separately

```bash
cargo owner --list <package> --registry <registry>
```

A named owner can publish, yank, and change owners. A crates.io team owner can publish and yank,
but cannot change owners. Prefer a team owner when the registry supports it.
Get authorization for one exact command, run only that command, then list the owners again and
report the final set:

```bash
cargo owner --add <account-or-team> <package> --registry <registry>
cargo owner --remove <account-or-team> <package> --registry <registry>
```

## Official references

- [crates.io policies](https://crates.io/policies)
- [`cargo yank`](https://doc.rust-lang.org/cargo/commands/cargo-yank.html)
- [`cargo owner`](https://doc.rust-lang.org/cargo/commands/cargo-owner.html)
- [RustSec contributing](https://rustsec.org/contributing.html)

# Dependency cooldowns

Cargo 1.100 can delay selection of a new registry version. Set a team-approved
age in `.cargo/config.toml`; three days is an example, not a security guarantee:

```toml
[registry]
global-min-publish-age = "3 days"
```

Run `cargo update --dry-run` and inspect the versions Cargo holds back before
using the policy. Then review the lockfile diff with the dependency gate in
[SKILL.md](../SKILL.md). A delayed release can still be malicious.

Locked builds keep versions already in `Cargo.lock`. Registry versions without
`pubtime` are not delayed. `cargo install` ignores this setting for the tool and
its dependencies. Pin and review installed tools separately. Older Cargo does
not provide this guarantee; run `cargo --version` in each resolver job.

For a reviewed urgent fix, override the policy for one command:

```bash
CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow cargo update -p <crate> --precise <version>
```

Review the selected source before a build. Record why the exception is needed.
The selected version stays in the lockfile after the override ends. A later
`cargo update` re-resolves dependencies and can select an older eligible version;
it does not preserve the exception as permanent policy.

Test policy changes with a local registry and an isolated `CARGO_HOME`. Publish
one old version and one current version of a harmless fixture crate. Use real
registry `pubtime` entries and valid crate checksums. Check these outcomes:

- Initial resolution selects the old version.
- The one-command exception selects the current version.
- A subsequent `cargo check --locked` keeps that selection without the override.
- A full `cargo update` applies the age policy again.
- A version without `pubtime` remains eligible.

Bind the fixture registry to loopback and pass no registry token or service
credential to it. These checks prove resolver behavior; they do not vet an
upstream package or replace source review.

Source: [Cargo publish age resolution](https://doc.rust-lang.org/1.100.0/cargo/reference/resolver.html#publish-age).

# crates.io Trusted Publishing

Read this when a CI workflow publishes a crate to crates.io, or when you change the crate's
publisher settings. Facts are as of 2026-09; the source links are at the end.

Contents: What it changes, Configure the publisher, Workflow, Token fallback, Verify before the
tag push, Sources.

## What it changes

- The CI job exchanges its OIDC identity for a crates.io token. The token expires after 30
  minutes, and the post step of `rust-lang/crates-io-auth-action` revokes it when the job ends.
  No long-lived registry secret exists in the repository.
- GitHub Actions is supported. GitLab.com CI/CD is in public beta. Self-hosted GitLab is not
  supported.
- The first publish of a new crate needs an API token. Configure the trusted publisher after it.
- crates.io rejects workflows that the `pull_request_target` or `workflow_run` trigger starts.
- An owner can enable "Trusted Publishing only" in the crate settings. That setting disables
  API-token publishing for the crate.

## Configure the publisher

The crates.io crate settings take the repository owner, the repository name, the workflow file
name (for example `release.yml`), and an optional environment. On GitLab.com they take the
namespace, the project, the workflow file path, and an optional environment.
A settings change is an owner-level external write. Get authorization for the exact values.
Enable "Trusted Publishing only" only after the workflow has published one version, and get
authorization for that change too.

## Workflow

Pin every action to a full commit SHA with a version comment. The `cargo-workflows` skill, when it
is installed, owns the pinning rule.

```yaml
name: Publish to crates.io
on:
  push:
    tags: ['v*']
jobs:
  publish:
    runs-on: ubuntu-latest
    environment: release
    permissions:
      contents: read
      id-token: write
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - name: Check that the tag matches the package version
        run: |
          version="$(cargo metadata --locked --format-version 1 --no-deps \
            | jq -r '.packages[] | select(.name == "<package>") | .version')"
          if [ "${GITHUB_REF_NAME#v}" != "$version" ]; then
            echo "tag $GITHUB_REF_NAME does not match <package> $version" >&2
            exit 1
          fi
      - uses: rust-lang/crates-io-auth-action@c6f97d42243bad5fab37ca0427f495c86d5b1a18 # v1.0.5
        id: auth
      - run: cargo publish --locked -p <package>
        env:
          CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}
```

- Give `id-token: write` only to the publish job.
- Add required reviewers to the `release` environment, and enter the same environment name in the
  crate settings. The environment then gates each publish.
- Start the job from a tag push or a manual dispatch. Limit the tag pattern to release tags. Run a
  manual dispatch on the tag ref: the version check fails on a branch name.
- Set `persist-credentials: false` on the checkout. Otherwise the checkout leaves the `GITHUB_TOKEN`
  in the git configuration, where a build script that `cargo publish` verifies can read it.
- Do not restore or save a shared build cache in the publish job. A job that can write a cache
  that pull requests read must not hold secrets.
- Run the release gates in an earlier job. The publish job runs `cargo publish` without
  `--allow-dirty` and without `--no-verify`.

## Token fallback

Use a stored token only when Trusted Publishing is not available: another registry, another CI
host, or the first publish. Store the token as a CI secret. Never echo it.

For crates.io, create a token with the `publish-update` scope (add `publish-new` only for a first
publish) and a crate-name pattern. crates.io tokens expire after 90 days by default, so an old CI
secret can start to fail authentication. Pass the token as `CARGO_REGISTRY_TOKEN`.

For another registry, pass the token as `CARGO_REGISTRIES_<NAME>_TOKEN` (the registry name in
upper case, with `-` as `_`), or use the credential provider that the registry documents.
`CARGO_REGISTRY_TOKEN` is the crates.io token only. Follow the token policy of that registry.

## Verify before the tag push

| Check | Evidence |
|---|---|
| Trigger | `on:` names a tag push or `workflow_dispatch`, not `pull_request_target` or `workflow_run` |
| Permissions | `id-token: write` on the publish job only |
| Pins | Every `uses:` has a 40-character SHA and a version comment |
| Environment | The job environment matches the crate settings and has required reviewers |
| Publisher | The crate settings list this repository, this workflow file, and this environment |
| Command | `cargo publish --locked -p <package>`, with no `--allow-dirty` or `--no-verify` |
| Version | The checkout has `persist-credentials: false`, and a step fails when the tag differs from the package version |

After the job, read its log, then confirm the version with
`cargo info <package>@<version> --registry crates-io`.

| Failure | Likely cause | Fix |
|---|---|---|
| Auth step fails on a new crate | The crate has no published version | Do the first publish with a scoped token |
| Auth step fails on an existing crate | Workflow file, repository, or environment differs from the settings | Correct the settings or the workflow; do not add a token secret as a workaround |
| Auth step fails with an OIDC error | `id-token: write` is missing | Add it to the publish job |
| Publishing fails only for some runs | `pull_request_target` or `workflow_run` started those runs | Use a tag push or a manual dispatch |

## Sources

- [crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing)
- [crates-io-auth-action](https://github.com/rust-lang/crates-io-auth-action)
- [crates.io development update, 2026-01](https://blog.rust-lang.org/2026/01/21/crates-io-development-update/)
- [crates.io development update, 2025-02](https://blog.rust-lang.org/2025/02/05/crates-io-development-update/)
- [crates.io token scopes, RFC 2947](https://rust-lang.github.io/rfcs/2947-crates-io-token-scopes.html)
- [Cargo registry tokens in config](https://doc.rust-lang.org/cargo/reference/config.html#registriesnametoken)
- [GitHub Actions cache and secrets advisory](https://blog.rust-lang.org/2026/09/21/github-actions-leaking-secrets-when-miri-output-is-cached/)

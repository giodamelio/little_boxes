# CI cache steps and an example nextest configuration

Contents:

- Cache steps for GitHub Actions
- Example `.config/nextest.toml`

## Cache steps for GitHub Actions

Pin each action to a full commit SHA, and keep the version in a comment.

- GitHub-hosted runners removed Node 20 on 2026-09-23. Use the node24 majors: `actions/checkout`
  v5+, `actions/cache` v5+, `actions/upload-artifact` v6+. `Swatinem/rust-cache` runs on node24
  from v2.9.0; v2.8.x and older use node20 or node16.
- Set `cache-on-failure: true`. A failed job still compiled dependencies, and the next run can
  reuse them.
- Never expose secrets to a job that writes a cache that pull requests can read. Tools can copy
  the environment into `target/`: until the 2026-09-22 nightly, `cargo miri` stored every
  environment variable there ([Rust security advisory, 2026-09-21](https://blog.rust-lang.org/2026/09/21/github-actions-leaking-secrets-when-miri-output-is-cached/)).
  Scope a secret to one step that does not run cargo or Miri into the cached `target/`, or do not
  cache `target/` in that job. Clear the cache and rotate the secret if a cached job had one.

```yaml
- uses: Swatinem/rust-cache@<full-commit-sha>   # v2.9.0 or later (node24)
  with:
    cache-on-failure: true
    workspaces: "<workspace-dir> -> target"

# Manual cache, when you need control over the key
- uses: actions/cache@<full-commit-sha>          # v5 or later (node24)
  with:
    path: |
      ~/.cargo/registry/index/
      ~/.cargo/registry/cache/
      ~/.cargo/git/db/
      <workspace-dir>/target/
    key: ${{ runner.os }}-cargo-${{ hashFiles('<workspace-dir>/rust-toolchain.toml', '<workspace-dir>/Cargo.lock') }}
```

## Example `.config/nextest.toml`

```toml
# Fail loudly on a nextest too old for the keys below.
nextest-version = { required = "0.9.131" }

[profile.default]
fail-fast = true
slow-timeout = { period = "60s" }
# Keep the opt-in tests out of the default lane.
default-filter = 'not test(/^network_integration_/)'

[profile.ci]
fail-fast = false
retries = 2
flaky-result = "fail"
slow-timeout = { period = "60s", terminate-after = 3 }

# Opt-in profile for tests that touch the network.
[profile.network-integration]
default-filter = 'test(/^network_integration_/)'

[test-groups]
network = { max-threads = 1 }

[[profile.network-integration.overrides]]
filter = 'test(/^network_integration_/)'
test-group = 'network'
```

The profiles implement the rules in `SKILL.md`: the network tests run only in an opt-in,
single-threaded profile, and `retries` is set only on the CI profile, with
`flaky-result = "fail"` (nextest 0.9.131+).

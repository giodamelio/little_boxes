# Binary Release Distribution

Read this when a release includes downloadable binaries or other built artifacts. Do not use it
for a registry-only crate release.

Contents:

- Keep release channels separate
- Freeze the distribution contract
- Build from fixed inputs
- Stage a safe archive
- Create release metadata
- Sign and verify final bytes
- Upload in a recoverable order
- Verify as a consumer
- Stop and recovery rules

## Keep release channels separate

A Cargo registry stores a source package. A hosted release stores built assets. The two can share
a version and a tag, but they have separate failure modes and permissions.

Record which actions the user authorizes:

- create and push the release tag;
- publish each registry package;
- create a hosted draft;
- upload each named asset;
- sign an artifact or publish an attestation;
- finalize or otherwise make the hosted release public.

Do not infer one authorization from another. Read the repository workflow before you act. Do not
run a local upload when the tag already starts an upload workflow.

## Freeze the distribution contract

Record this matrix before the build:

| Field | Required value |
|---|---|
| Source | Exact commit and release tag |
| Toolchain | Rust toolchain, installed components, linker, native libraries, and external tool versions |
| Build | Locked command, profile, features, and relevant environment |
| Target | Exact Rust target triple and supported CPU or system baseline |
| Runner | Native or existing cross-build environment |
| Asset | Exact immutable file name and archive format |
| Contents | Explicit files and expected executable name |
| Metadata | Checksum, SBOM, provenance, and signature policy |

Name an asset with the product, version, and exact target, for example
`<product>-<version>-<target>.<archive-extension>`. Use the complete triple when it carries an ABI
or C runtime distinction. Do not publish a mutable `latest` asset as the only download. Do not
call a target supported until its release artifact passes its target-specific checks.

A static `*-linux-musl` binary embeds the vendored musl. Rust 1.95 patched CVE-2026-6042 and
CVE-2026-40200 in it, so build musl release assets with 1.95 or later.

## Build from fixed inputs

Use a clean checkout of the approved commit and the repository's release command. When no release
command exists, start with:

```bash
cargo build --locked --release --target <target> -p <package> --bin <binary>
```

Use the exact supported feature set, not `--all-features` by habit. Do not copy an artifact from
an earlier build or a developer target directory.

Separate these claims:

- **Deterministic packaging:** the same files and normalized archive metadata produce the same
  archive bytes.
- **Reproducible build:** independent builds of the same source and declared inputs produce
  byte-identical artifacts.

Two clean directories on one runner prove local repeatability only; they share the toolchain,
cache, environment, and undeclared host inputs. Before you claim reproducibility, build on
independently provisioned builders and compare the unpacked payload digests and the archive
digests.

Path inputs that change the bytes:

| Input | Control |
|---|---|
| Absolute source paths | `--remap-path-prefix=<dir>=<stable-name>`; `--remap-path-scope` (stable since 1.95) limits where it applies (`macro`, `diagnostics`, `debuginfo`, `coverage`, `object`, `all`) |
| The `rust-src` component | Since 1.83, an installed `rust-src` makes rustc embed its local path instead of `/rustc/<hash>`. Remap that path too, or build without `rust-src` on every builder |
| Linker-embedded paths | `--remap-path-prefix` does not reach them (MSVC `.pdb`, Apple OSO entries; Apple needs `-oso_prefix`) |
| Timestamps | Set `SOURCE_DATE_EPOCH` to the commit time for archive and SBOM tools that honor it |

Cargo `trim-paths` is nightly-only as of Rust 1.99.1: do not use it in a stable release build.
Apply remapping only when the repository requires it or the release process controls all
affected tools.

## Stage a safe archive

Create a fresh staging directory with one versioned top-level directory. Copy only an explicit
allowlist, such as the executable, README, license, and required runtime files. Do not copy the
repository root or a complete build directory.

Set stable file order, timestamps, owner and group metadata, and permission bits with the
repository's archive command. Keep the executable bit on Unix targets. GNU, BSD, and Windows
archive tools take different flags. Pin the runner and record the archive tool version when exact
archive bytes matter.

Run a programmatic gate over every archive entry and link target before extraction. Reject:

- POSIX absolute paths, Windows drive paths, UNC paths, and device paths;
- `..` traversal under both `/` and `\` separators;
- symbolic or hard links whose targets escape the archive root;
- device nodes, sockets, other special files, and invalid Windows names, alternate data streams,
  and reserved names;
- secrets, credentials, private keys, signing material, and CI state;
- debug files or native libraries that the release contract does not include.

Only after that gate passes, extract into a new disposable directory with owner and permission
restoration disabled. Assert that every canonical extracted path stays below that directory.
Listing commands help a human review, but they are not the safety gate:

```bash
tar -tvf <asset>.tar.gz
unzip -Z1 <asset>.zip
```

Run the packaged executable from that directory, on the matching target environment, not from
`target/`:

```bash
<extracted-path>/<binary> --version
<repository-smoke-command> <extracted-path>/<binary>
```

Check the required shared libraries and system baselines there.

## Create release metadata

Generate one checksum manifest over every final archive and SBOM. Leave out the manifest itself,
its signatures, and the provenance, because the provenance is made from the manifest. Use SHA-256
unless policy requires another approved digest. Sort by asset name and keep the
`<digest><two spaces><file>` format. Generate it with the release command or the platform checksum
tool; never edit a digest by hand.

Generate an SBOM for each released artifact, not only for the source workspace, with the
repository's CycloneDX or SPDX generator. An SBOM lists contents and dependencies; it does not
prove where the artifact was built. With `cargo-cyclonedx`:

- pass `--target <triple>`; the default is the host target, which is wrong for cfg-gated
  dependencies of other targets;
- pass `--spec-version 1.5` when consumers need it; the default is 1.3;
- pass `--describe binaries` for one SBOM for each binary or cdylib;
- set `SOURCE_DATE_EPOCH` (0.5.9 or later); otherwise each run writes a new timestamp and a
  random serial number, and the checksum manifest changes.

Validate each SBOM with the repository's schema check when one exists.

Generate provenance after the checksum manifest, with the configured build platform, and bind it
to the final artifact digests.
Do not claim a SLSA level that the platform and workflow do not establish. A signature is not
provenance. Plain provenance JSON without an authenticated envelope is metadata, not verified
provenance.

On GitHub Actions, `actions/attest@v4` writes build provenance by default
(`actions/attest-build-provenance` v4 is a wrapper around it). It works for public repositories
on all plans, for private repositories only on GitHub Enterprise Cloud, and not on GitHub
Enterprise Server. Pin it to a full commit SHA:

```yaml
permissions:
  contents: read
  id-token: write
  attestations: write
  artifact-metadata: write
steps:
  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2
    with:
      subject-checksums: SHA256SUMS
```

`subject-checksums` attests every file that the manifest lists. `sbom-path` attests an SPDX or
CycloneDX JSON SBOM. GitHub stores the attestation, and consumers check it with
`gh attestation verify`. The attestation has its own signature. If you also upload the provenance
bundle as an asset, that signature authenticates it; the manifest signature does not cover it.

When the repository already builds with `cargo auditable`, `cargo audit bin <binary>` audits the
shipped dependency list exactly. Without it, `cargo audit bin` recovers only part of the list.
Do not add a new SBOM, signing, or attestation tool only for this release. If the repository has
no approved mechanism, report the missing control.

## Sign and verify final bytes

Use one closed authentication policy. The default: sign the final checksum manifest that covers
every archive and SBOM. Consumers verify that signature and identity before they trust any listed
digest. Consumers verify provenance through its own signature. Use per-file signatures only when
policy requires them, and leave no downloaded file outside the authenticated set.

Sign only final files. Use the repository's key, identity-based workflow, hardware token, or
signing service. Never create or export a signing key without explicit authorization.
Identity-based signing writes to a public transparency log; get authorization first.
In CI, pass `--yes` to `cosign sign-blob` only after that authorization. The flag skips the
prompt that confirms the public transparency-log upload.

cosign v3 requires `--bundle` for `sign-blob` and writes the new bundle format by default.
Scripts written for v2 `--output-signature` and `--output-certificate` files do not match it.

```bash
cosign sign-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json
cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
  --certificate-identity "https://github.com/<owner>/<repo>/.github/workflows/<release>.yml@refs/tags/<tag>" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

Run the verification once with a wrong `--certificate-identity`. It must fail; a check that also
passes for the wrong identity checks nothing.

Verify against an established trust policy: the expected key or certificate identity and issuer,
the exact bytes, the required transparency proof or bundle, and the expected provenance subject
and builder. Do not trust a public key only because it comes from the same release as the
signature. Keep the bundles, certificates, and attestations that consumers need for offline
verification.

## Upload in a recoverable order

Verify all local assets before the first upload. Create a draft or other non-final release when
the host and workflow support it.

When a repository or organization enables GitHub immutable releases, publishing the draft locks
the tag and every attached asset against change or deletion. Attach every file to the draft
before you publish it.

Upload in this order:

1. Upload the immutable binary archives.
2. Upload each SBOM that names those archives.
3. Upload the checksum manifest.
4. Upload the signatures, certificates, bundles, and provenance bundles.
5. Download the draft assets through the host's authenticated endpoint and verify them.
6. Make the release final only after every check passes and finalization is authorized.
7. Download every file again through the public URL, without credentials, and repeat the digest,
   signature, provenance, archive, and smoke checks.

Never overwrite an asset with different bytes. After an upload timeout, refresh the asset list a
bounded number of times, then decide per asset:

| Observed remote state | Action |
|---|---|
| Downloaded remote digest equals the local digest | Accept the upload. Do not retry. |
| Asset is absent after the bounded refresh | Get authorization before one retry of the identical bytes. |
| Digest differs, the download is ambiguous, or different bytes use the name | Stop. Do not overwrite or delete without exact authorization. Use a new version after publication. |

## Verify as a consumer

For a draft, download every file through the authenticated endpoint into a new directory. After
finalization, repeat the check through the public path without release credentials. Never verify
the local upload source in place of a download.

Check all of these facts:

- the release tag points to the approved commit;
- the remote asset set matches the target matrix exactly;
- the manifest signature, or every required per-file signature, passes the identity policy
  before any listed digest is trusted;
- each downloaded digest matches the authenticated checksum manifest;
- each attestation passes its builder and source policy, and every subject matches a downloaded
  digest;
- each SBOM names the expected product, version, and target;
- every archive passes the path and link gate;
- every extracted binary reports the expected version and passes its smoke check.

For GitHub attestations, pin the builder and the source:

```bash
gh attestation verify <asset> --repo <owner>/<repo> \
  --signer-workflow <owner>/<repo>/.github/workflows/<release>.yml \
  --source-ref refs/tags/<tag> --source-digest <commit> \
  --deny-self-hosted-runners
```

For a published immutable release, `gh release verify <tag>` checks the release attestation, and
`gh release verify-asset <tag> <file>` checks one downloaded file against it.

Record the public asset URLs and the observed digests in the completion report.

## Stop and recovery rules

Stop the release sequence when a required target fails, an archive has an unsafe entry, a digest
differs, a signature has the wrong identity, provenance names the wrong source or artifact, or an
upload result is uncertain.

| Symptom | Likely cause | Fix |
|---|---|---|
| Two clean binary builds have different digests | Path, timestamp, linker, toolchain component, or archive metadata differs | Find the variation or remove the reproducibility claim |
| Binary upload times out | The host can have accepted the asset | Download and hash the remote bytes; follow the retry table above |
| Signature verifies for an unexpected identity | The signature is valid but the trust policy is wrong | Stop and pin the expected identity and issuer |

Keep an incomplete release non-final when the workflow supports that state. Do not delete or hide
a public release without explicit authorization. For changed bytes, create a new version and
explain the superseded release. If a secret or signing key enters an asset, revoke it
immediately, then contact the release host.

## Official references

- [Cargo build](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
- [rustc source path remapping](https://doc.rust-lang.org/rustc/remap-source-paths.html)
- [Rust release notes](https://doc.rust-lang.org/stable/releases.html)
- [SOURCE_DATE_EPOCH](https://reproducible-builds.org/specs/source-date-epoch/)
- [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo)
- [cargo audit bin](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
- [actions/attest](https://github.com/actions/attest)
- [gh attestation verify](https://cli.github.com/manual/gh_attestation_verify)
- [GitHub immutable releases](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases)
- [Sigstore blob signing](https://docs.sigstore.dev/cosign/signing/signing_with_blobs/)
- [Sigstore signature verification](https://docs.sigstore.dev/cosign/verifying/verify/)
- [cosign changelog](https://github.com/sigstore/cosign/blob/main/CHANGELOG.md)
- [SLSA provenance](https://slsa.dev/spec/v1.2/provenance)
- [CycloneDX specification](https://cyclonedx.org/specification/overview/)
- [SPDX specification](https://spdx.dev/use/specifications/)

# Security Policy

## Supported versions

`diaspor` is pre-1.0 software. Only the latest released minor version
on the `main` branch receives security fixes. Once v1.0 ships (M6), the
last two minor versions are supported.

## Reporting a vulnerability

**Do not open a public issue** for a suspected vulnerability. Email
**anthonypaquet1508@gmail.com** with subject `[diaspor security]`.

Please include:

- A description of the issue.
- Steps or a minimal reproducer.
- Affected crate(s) and version(s).
- The impact you believe it has.

You should expect an acknowledgement within seven days and a remediation
plan within thirty. Fixes are released as patch versions and noted in
`CHANGELOG.md` with a CVE reference where applicable.

## Out of scope

- Vulnerabilities in third-party dependencies (FFmpeg, whisper.cpp,
  llama.cpp, the platform FUSE/WinFsp runtimes). Report those upstream;
  we will track and update.
- Misconfiguration of a downstream application that embeds the library.
- Theoretical attacks that require an attacker who already has full
  local access to the device the VFS runs on.

## Privacy contract

The privacy contract of `diaspor-index` (no audio bytes leave the
device unless the operator explicitly configures a cloud transcriber)
is part of this security policy. A bug that causes the default
pipeline to make an unsolicited network call is a security issue and
will be treated with the same severity as a remote code execution.

The same applies to `diaspor-infer`'s `ModelHub`: when the environment
variable `DIASPOR_OFFLINE=1` is set, no model weights may be fetched
from the network — the hub must error out with `NetworkBlocked`. The
`no-network` job in `.github/workflows/ci-rust.yml` runs the full test
suite inside an `unshare(--net)` sandbox to enforce this structurally.

## Release artifact integrity

Every release artifact published to
`https://github.com/stonyp90/diaspor/releases` is shipped with:

1. **SHA-256 checksum** — `<archive>.sha256` next to every archive.
2. **Minisign signature** — `<archive>.minisig` produced by an
   Ed25519 key held only as a GitHub Actions secret. Verifiable
   end-to-end with `minisign -Vm <archive> -P <public-key>` (or
   `rsign verify`).
3. **SLSA build-provenance attestation** — an in-toto attestation
   bound to the GitHub Actions workflow run, queryable via
   `gh attestation verify <archive> --owner stonyp90`.

### Public key

The minisign public key is published below and inside the GitHub
release notes for every tagged release. **Pin this key locally** — do
not trust a key you fetch over plain HTTP from the same origin as the
artifact.

```
# diaspor minisign public key (Ed25519)
# (placeholder — replace at first release with the actual key once
# `rsign generate` has been run; key fingerprint will be quoted in the
# GitHub release notes)
RWQ-NOT-YET-GENERATED-RUN-RSIGN-GENERATE-AND-COMMIT-THIS-VALUE
```

### Verifying a release manually

```bash
# 1. Download archive + .sha256 + .minisig
gh release download v0.1.0-alpha.2 \
  -p 'diaspor-*-aarch64-apple-darwin.tar.gz' \
  -p 'diaspor-*-aarch64-apple-darwin.tar.gz.sha256' \
  -p 'diaspor-*-aarch64-apple-darwin.tar.gz.minisig'

# 2. Check SHA-256
shasum -a 256 -c diaspor-*-aarch64-apple-darwin.tar.gz.sha256

# 3. Verify the Ed25519 signature (requires `rsign` or `minisign`)
cargo install --locked rsign2  # one-time
rsign verify diaspor-*-aarch64-apple-darwin.tar.gz \
  -P "$(cat SECURITY.md | sed -n '/^RWQ/p')"

# 4. Verify the SLSA build provenance attestation
gh attestation verify diaspor-*-aarch64-apple-darwin.tar.gz \
  --owner stonyp90
```

### Operator setup for the release pipeline

For a maintainer rolling the keys:

```bash
# One-time, on a clean machine:
cargo install --locked rsign2
rsign generate -p public.key -s rsign.key  # uses a passphrase
# Add the *contents* of rsign.key to the repo secret MINISIGN_SECRET
# Add the passphrase to the repo secret MINISIGN_PASSWORD
# Commit the *contents* of public.key in place of the placeholder above
```

When `MINISIGN_SECRET` is not set, `.github/workflows/release.yml`
still publishes the archives + SHA-256 + SLSA attestation. The
workflow emits a `::warning` to make the unsigned state visible on
the run page. This is a temporary degradation, not a silent failure.

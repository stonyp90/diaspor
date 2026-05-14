# Security Policy

## Supported versions

`cairn` is pre-1.0 software. Only the latest released minor version
on the `main` branch receives security fixes. Once v1.0 ships (M6), the
last two minor versions are supported.

## Reporting a vulnerability

**Do not open a public issue** for a suspected vulnerability. Email
**anthonypaquet1508@gmail.com** with subject `[cairn security]`.

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

The privacy contract of `cairn-index` (no audio bytes leave the
device unless the operator explicitly configures a cloud transcriber)
is part of this security policy. A bug that causes the default
pipeline to make an unsolicited network call is a security issue and
will be treated with the same severity as a remote code execution.

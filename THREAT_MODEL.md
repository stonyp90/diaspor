# Threat model

`stony-vdfs` is a library, not a product. The threat model below
describes which threats the library *itself* mitigates, which are
*deferred to integrators*, and which are *explicitly out of scope*.

## Adversary capabilities considered

- **Untrusted media in the VFS.** A media file may be deliberately
  malformed (over-long metadata fields, malicious codecs, oversized
  audio streams) to exploit FFmpeg, whisper.cpp, or llama.cpp.
- **Compromised dependency upstream.** A supply-chain attack on one
  of our direct or transitive Rust crates (e.g. a typo-squat or a
  malicious release).
- **Local-but-unprivileged user code.** Other processes on the host
  may try to read sidecar JSON files containing transcripts.
- **Network-side eavesdropper.** Someone watching the host's network
  traffic, on the assumption they expect to see audio uploads.

## Adversary capabilities NOT considered

- **Privileged local attacker (root, kernel, hypervisor).** Out of
  scope: at that capability level the attacker can read process memory
  directly.
- **Physical-access attacker.** Out of scope.
- **GPU-memory side channels.** Out of scope at the library layer; if
  whisper.cpp / llama.cpp leak through GPU memory, the issue is upstream.
- **Acoustic side channels** (microphone leakage). Out of scope.

## Mitigations the library implements

| Threat                                  | Mitigation                                                             |
|-----------------------------------------|-------------------------------------------------------------------------|
| Malformed media exploits FFmpeg         | FFmpeg invoked as a *subprocess*, not linked in-process; subprocess crash does not kill the host. Resource limits applied (max duration, max file size). |
| Malformed audio exploits whisper.cpp    | Same subprocess isolation; PCM buffer size capped before handoff.       |
| Tagger prompt-injection from transcript | Transcript is JSON-escaped, not embedded as free text in the prompt; LLM is invoked with a fixed system prompt and short user message. |
| Supply-chain attack on a Rust dep       | `cargo-deny` checks bans + licences + sources + advisories on every CI run; `Cargo.lock` is committed and reviewed. |
| Network exfiltration of audio           | "No-network" CI job runs the default pipeline in `unshare -n`; any default-path socket call fails the build. |
| Sidecar JSON readable by other local processes | Library writes sidecars with `0600` permissions on POSIX; on Windows, default ACL. Integrators must verify their backend honours these. |
| Symlink attacks via local backend       | M2 implements an "unresolved symlinks" mode that refuses to traverse symlinks pointing outside the configured root. |

## Threats deferred to integrators

The library cannot enforce these — the integrating application must:

- **Restrict the backend's root.** If your `LocalBackend` is mounted at
  `/` you have given the library the whole filesystem.
- **Sandbox the FFmpeg / whisper.cpp / llama.cpp binaries.** Use OS
  primitives (seccomp, AppArmor, Windows AppContainer) for defence in
  depth. The library does the in-process part; the integrator does the
  host part.
- **Authenticate downstream callers.** If your application exposes a
  network API that wraps `stony-vdfs`, *your* layer authenticates.

## What we recommend integrators read alongside this file

- `PRIVACY.md` — the data-flow guarantees that complement the threat
  model.
- `SECURITY.md` — how to report a suspected vulnerability.
- The FFmpeg, whisper.cpp, and llama.cpp upstream security pages —
  for the parts of the threat model we delegate to them.

## Updating this document

A material change to the threat model (e.g. linking FFmpeg in-process
instead of subprocess, removing the no-network CI job) requires a
companion ADR in `docs/adr/` and a note in `RELEASES.md` flagging the
release that introduced the change.

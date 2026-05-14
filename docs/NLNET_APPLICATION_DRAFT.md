# NLnet NGI Zero Commons Fund — Application Draft

**Call:** 13th NGI Zero Commons Fund call  
**Deadline:** 2026-06-01 12:00 CEST (Brussels time)  
**Applicant:** Anthony Paquet (sole maintainer)  
**Project:** `stony-vdfs`  
**Requested budget:** €45 000 (≈ CAD 60 000) over 12 months  
**Repository:** https://github.com/stonyp90/stony-vdfs (primary) + https://codeberg.org/stonyp90/stony-vdfs (EU mirror)  
**License:** MIT

> This document is the draft text for the **public** NLnet application form. Once
> letters of support are confirmed (see `docs/EU_COAPPLICANTS.md`), paste the
> matching sections into the corresponding fields on
> https://nlnet.nl/propose/ . Numbers in headings track the NLnet form fields as
> of the 12th call announcement; the 13th call form is structurally identical.

---

## 1. Project name

`stony-vdfs` — a privacy-first Rust virtual filesystem with on-device FFmpeg
transcription and local-LLM auto-tagging.

## 2. Website / wiki

- https://github.com/stonyp90/stony-vdfs
- https://codeberg.org/stonyp90/stony-vdfs (EU sovereign mirror)

## 3. Abstract (≤ 1200 characters)

`stony-vdfs` is a Rust library that gives applications a single async API over
multiple storage backends (memory, local disk, FUSE, WinFsp) — plus an opt-in
content-understanding layer that runs entirely on the user's device. When a
media file lands in the filesystem, `stony-vdfs-index` probes it with FFmpeg,
transcribes the audio with `whisper.cpp`, and auto-tags the transcript with a
small local LLM. Transcripts and tags persist as sidecar JSON files retrievable
through the VFS itself. No cloud calls, no telemetry, no API keys. The
end-user experience matches Google Drive / OneDrive media search ("find the
recording where we discussed pricing") with the privacy trade-off inverted:
nothing leaves the device. The library is written from a clean specification
with no source-code or asset reuse from prior closed work the applicant has
done in adjacent media-pipeline engineering. The NLnet grant funds the work
of taking the architecture from a public alpha to a stable v1.0 library
that any application or operating system distribution can adopt.

## 4. Have you been involved with projects or organisations relevant to this project before? (≤ 2500 characters)

Yes. The applicant has spent 2022–2025 building closed-source desktop
applications in the same domain (cross-platform media pipelines on
Linux/macOS/Windows with FFmpeg and Whisper components). That commercial
experience surfaced the gap that `stony-vdfs` addresses — the absence of
an embeddable, MIT-licensed Rust crate that wires on-device transcription
to a filesystem surface. The open-source library is being written from a
clean specification in Rust 2024 edition, with no source-code or asset
reuse from prior closed work; the applicant holds full rights to license
the new code under MIT and no third party (employer, consortium, prior
investor) has IP claims on it.

The applicant has read upstream issues and tested patches in the
`tokio`, `bytes`, FFmpeg Rust binding, and `whisper.cpp` ecosystems
while building production code that depends on them. Specific public
contributions are linked from the GitHub profile
(`github.com/stonyp90`). The applicant has not previously applied to
NLnet. Letter-of-support outreach to Codeberg e.V. and FSFE (Berlin)
is in progress; see `docs/EU_OUTREACH_EMAILS.md` for the sent emails.

## 5. Requested amount (€)

**€45 000** (forty-five thousand euros) over 12 months, milestone-paid in
six tranches of €7 500 each on delivery of M1–M6.

## 6. Explain what the requested budget will be used for (≤ 5000 characters)

The budget funds **one developer (the applicant) at ~20 hours per week
(50 % FTE) for 12 months** — the minimum cadence at which the project
can reach a stable v1.0 release inside the grant window. The hourly
rate implied (20 h/week × 4.33 weeks/month × 12 months = 1 040 hours;
€45 000 / 1 040 ≈ €43/h) is below market for senior Rust systems
development; it is calibrated to NLnet's grant size, not to commercial
billing.

No budget is allocated to:

- Subcontractors (the applicant performs all work).
- Hardware (existing developer hardware is sufficient; CI runs on free
  GitHub Actions Linux runners and a small WinFsp self-hosted VM that the
  applicant maintains at no charge).
- Marketing, travel, or office overhead.
- Cloud services (the project is **explicitly local-first**; CI runs on
  ephemeral runners and uses no paid SaaS).

Six concrete milestone deliverables, each tied to a €7 500 tranche:

- **M1** (Month 1–2) — Core API frozen + memory backend at 100 % +
  publishable conformance test suite. Tag `v0.1.0`.
- **M2** (Month 3–4) — Local-disk backend meets explicit acceptance
  criteria: symlink handling, extended attributes, the long tail of
  cross-platform path quirks, ≥ 85 % line coverage, CI green on
  Linux/macOS/Windows. Tag `v0.2.0`.
- **M3** (Month 5–6) — FUSE adapter end-to-end on Linux and macOS, with
  Linux being the primary target (FUSE is mature and unencumbered there).
  Tag `v0.3.0`.
- **M4** (Month 7–8) — WinFsp adapter end-to-end on Windows. Tag `v0.4.0`.
- **M5** (Month 9–10) — `stony-vdfs-index` ships the **FFmpeg + whisper.cpp**
  transcription pipeline as an opt-in crate, with a deterministic CLI demo
  (`stony-vdfs transcribe input.mp4 -o sidecar.json`). Tag `v0.5.0`.
- **M6** (Month 11–12) — **Local-LLM auto-tagging** via a small GGUF model
  through `llama.cpp`, sidecar persistence in the VFS itself, documentation
  pass, and a tagged **v1.0** release. Tag `v1.0.0`.

The roadmap is intentionally **VFS-first, AI-optional**: M1–M4 deliver a
library that meets measurable acceptance criteria and is useful on its
own; M5–M6 add the content-understanding layer as an opt-in crate. This
phasing protects NLnet against the risk that the AI tooling chain
(FFmpeg, whisper.cpp, llama.cpp) introduces friction late in the grant;
the core deliverable is independent of it.

## 7. Does the project receive (other) funding? (≤ 2500 characters)

No active matching grants. The applicant has not previously applied
to NLnet. The `stony-vdfs` open-source work funded by NLnet would be
reported separately from any other R&D activity the applicant
undertakes, with no cost double-attribution. The applicant is happy to
provide a written IP attestation under separate cover if NLnet's review
panel requests one.

## 8. Compare your own project with existing or historical efforts (≤ 2500 characters)

Three layers of comparison:

**Cross-platform VFS abstractions in Rust:** `vfs` (msr-cogito,
synchronous), `virtual-fs` (wasix, WASM-flavoured), `opendal` (Apache,
object-store first, optional FUSE adapter but no WinFsp), `cap-std`
(capability-based fs, no FUSE/WinFsp surface), `fuser` (Linux/macOS
only), `polyfuse` (async but Linux-only), `winfsp-rs` and `winfsp-sys`
(Windows only), `dokany-rust` (Windows, less maintained than WinFsp),
`nfsserve` (NFSv3 server, network protocol focus). C/C++ historical
comparators: libcfu, GVfs (GNOME, GPL, server-tied), Dokany (Windows,
fragmented governance). The closest active prior art is `opendal` +
manual FUSE/WinFsp wrapping, which still leaves the integrator with
two adapter stacks to maintain. `stony-vdfs` ships the integration as
a single async Rust workspace under one permissive licence.

**Transcription stacks:** Cloud-side — OpenAI Whisper API, AssemblyAI,
Deepgram, Otter.ai, Descript, Rev, Microsoft Teams (Azure Speech),
Google Meet, Zoom AI Companion — all upload audio. On-device-but-closed —
Apple Intelligence speech, Microsoft Recall, Pixel Recorder, Samsung
Galaxy AI. On-device-and-open but framework-level — `whisper.cpp` (CLI),
`whisper-rs` (FFI crate), `faster-whisper` (Python/CTranslate2), Vosk,
Coqui STT, Mozilla DeepSpeech (archived). LinTO (Linagora) is the
closest EU comparator but is a server product, not an embeddable
library. The gap `stony-vdfs-index` fills is **the glue that wires
on-device transcription to a filesystem surface** so any application
ships Drive-style search without leaving the device.

**Local LLM tagging + persistence:** Closest comparators are `llamafile`,
Ollama, `llama.cpp` server, LM Studio, GPT4All — all are runtimes, not
filesystem-integrated taggers. Tantivy, Meilisearch, Quickwit are local
full-text indexes but operate on already-extracted text. Recoll and
Apache Tika do desktop indexing but assume cloud or local plaintext
input. No existing project closes the loop: media → transcript → tags →
filesystem sidecar → queryable through the same async API regardless of
backend.

## 9. Are there any other free and open source projects with similar goals? (≤ 2500 characters)

Yes, and naming them is part of why the project is differentiated rather
than redundant.

- **Forgejo / Codeberg** — Git hosting cooperative for FOSS code, used as a
  mirror target by this project. Different problem domain.
- **Nextcloud** — cloud collaboration platform that ships an indexer; their
  indexer is server-side, runs on PHP, and is tied to the Nextcloud stack.
  `stony-vdfs-index` runs in-process, in Rust, with no server.
- **rclone** — multi-backend cloud storage CLI. Operates on remote stores,
  not on a local FUSE/WinFsp surface, and has no transcription path.
- **Subtitle Edit / Whisper.cpp wrappers** — desktop GUIs that wrap
  whisper.cpp for one-off transcription. They are end-user tools, not
  libraries; an application cannot embed them as a reusable VFS layer.
- **Mediasoup-style indexers** — niche academic projects with strong VFS
  ideas (e.g. content-addressable storage) but no maintained Rust crate.

There are no current FOSS projects with the same combined scope. Past
NGI-funded filesystem work (the 9P, FUSE, and container-image
filesystem layers) addresses orthogonal problems — network transport,
mount-time performance, image layering — not user-facing content
understanding. `stony-vdfs` is complementary to that body of work.

## 10. Explain how this project advances the state-of-the-art (≤ 2500 characters)

Three concrete differentiators against named prior art:

1. **Single MIT-licensed Rust workspace combining (a) async VFS trait,
   (b) FUSE adapter, (c) WinFsp adapter, (d) sidecar-persistent local
   transcription, and (e) local-LLM auto-tagging.** Each ingredient
   exists separately — `opendal` (async VFS, no FUSE on the same surface
   as WinFsp), `fuser` (FUSE only), `winfsp-rs` (WinFsp only),
   `whisper-rs` (transcription library, no filesystem surface),
   `ffmpeg-next` (FFmpeg bindings without pipeline). `stony-vdfs` is the
   composition of those primitives into a single embeddable crate under
   one permissive licence, with the privacy contract enforced at the
   architectural layer rather than left to integrators.

2. **Privacy contract testable in CI.** The default index pipeline runs
   inside a network-disabled sandbox (`unshare -n` on Linux runners) as
   a release-gating job; any code path that opens a socket on the default
   pipeline fails the build. To our knowledge no other Whisper/LLM
   library makes this assertion mechanically verifiable rather than
   policy-asserted.

3. **Architecture small enough to audit.** `stony-vdfs-index` is
   designed to stay under 5 000 lines including tests; downstream
   security auditors and EU public-sector procurement reviewers can
   read the complete data-flow path in an afternoon. Cloud transcription
   services require trust in tens of millions of lines of closed code
   running in foreign jurisdictions.

## 11. The team (≤ 2500 characters)

**Anthony Paquet — sole maintainer.** Senior Rust / Tauri / FFmpeg
developer based in Quebec, Canada. ~10 years of professional software
engineering across systems programming, desktop applications, and media
pipelines on Linux, macOS, and Windows. The cross-platform pain that
`stony-vdfs` solves is pain the applicant has paid years of debugging
time on while shipping commercial media tooling.

Verifiable artefacts NLnet reviewers can check independently:

- **GitHub profile:** github.com/stonyp90 — public commit history,
  upstream issue activity in the Rust ecosystem (`tokio`, `bytes`,
  FFmpeg bindings, `whisper.cpp`), and the `stony-vdfs` repository
  itself with public commit history prior to submission.
- **Cross-platform shipping experience:** prior commercial work spans
  Linux, macOS, and Windows simultaneously — the same target matrix
  that `stony-vdfs` solves at the library layer.
- **IP attestation available on request:** the applicant holds full
  rights to license the new `stony-vdfs` code under MIT; no third
  party (prior employer, consortium, or investor) holds claims on it.
  A signed attestation is available to NLnet's review panel under
  separate cover if requested.

A small EU advisory circle is being formed: outreach completed with
Codeberg e.V. (Berlin) and FSFE (Berlin/Hamburg), with further contacts
in progress at Linagora/LinTO (Paris), KDE e.V. (Berlin), and Funkwhale
(NLnet alumnus, Germany) — see `docs/EU_COAPPLICANTS.md` and
`docs/EU_OUTREACH_EMAILS.md`.

**Year-1 community-growth plan.** The project ships with the
contributor infrastructure NLnet expects from a sustainable open
project from day one: GitHub issue templates (bug / feature / docs)
with `good first issue` pre-labelling, a 19-line PR template,
CODEOWNERS, FUNDING.yml, an enforced Code of Conduct (Contributor
Covenant v2.1), and a published `stony-vdfs-conformance` crate that
gives external backend authors a public way to contribute back. Target
trajectory through M6: at least three regular non-author contributors
visible in `git log`, with per-crate ownership formally added to
CODEOWNERS as contributors become regulars. Outreach roster across
Mastodon, r/rust, r/selfhosted, HN Show HN, Codeberg fediverse, and
the NLnet alumni network is staged through `docs/COMMUNITY_OUTREACH_DRAFTS.md`.

## 12. Do you have any (significant) European linkages? (≤ 1200 characters)

The applicant is Canadian, not EU-resident. The project's European
linkage is delivered, not promised:

1. **Live Codeberg mirror** at `codeberg.org/stonyp90/stony-vdfs`
   (Berlin, Germany) — set up before submission, syncing tagged
   releases automatically. Codeberg is the chosen *primary* EU
   distribution channel for binary releases.
2. **Codeberg e.V. and FSFE letter-of-support outreach** —
   personalised emails sent (see `docs/EU_OUTREACH_EMAILS.md`); at
   least one signed Letter of Support targeted for submission.
3. **EU policy alignment by architecture**: GDPR's data-minimisation
   principle is enforced *structurally* (no audio bytes leave the
   device); the design defuses Schrems-II / US CLOUD Act exposure for
   EU public bodies adopting transcription tooling. A "Deployment for
   EU public bodies" guide ships in M6.
4. **EU AI Act readiness:** the project's privacy contract anticipates
   the August-2026 entry-into-force of the EU AI Act's transparency
   and data-governance obligations for general-purpose AI deployments.

## 13. Risks and mitigation (≤ 2500 characters)

| Risk                                       | Mitigation                                                                                       |
|--------------------------------------------|--------------------------------------------------------------------------------------------------|
| Solo maintainer becomes unavailable (illness, life event) | Trait-driven architecture, MIT licence, public roadmap, and milestone-gated commits mean any Rust developer can fork at any tagged release; the published `stony-vdfs-conformance` crate ensures external backends can verify themselves without applicant involvement; Year-1 plan targets ≥ 3 regular non-author contributors and per-crate CODEOWNERS, lifting bus-factor from 1 to 4+ inside the grant window; NLnet's milestone-based disbursement also caps grant exposure to the last completed milestone, protecting NLnet from sunk-cost loss. |
| FFmpeg / whisper.cpp / llama.cpp upstream regressions | All three dependencies are subprocess- or FFI-wrapped, never statically linked into the core; M5–M6 pin specific commits in `Cargo.lock` and document a tested binary matrix; bring-your-own-binary is the default. |
| WinFsp licence ambiguity (GPLv3 + commercial dual-licence) | The `stony-vdfs-winfsp` crate is feature-gated and isolated in its own workspace member; downstream MIT-only packagers can omit it entirely and still ship FUSE + memory + local backends. |
| AI tooling chain (whisper.cpp/llama.cpp APIs) shifts mid-grant | VFS-first phasing: M1–M4 deliver a stand-alone production library worth €30 000 of value if M5–M6 must be re-scoped. Trait surface for index pipeline is decoupled from any specific runtime. |
| European-dimension review filter rejects non-EU solo applicant | Live Codeberg mirror at submission (not promised — delivered); active LoS outreach to Codeberg e.V., FSFE, Linagora/LinTO, KDE e.V., Funkwhale (NLnet alumnus) per `docs/EU_COAPPLICANTS.md`; targeted Fediverse outreach to attract EU-based contributors before submission (Codeberg forum, Funkwhale and Framasoft communities — see `docs/COMMUNITY_OUTREACH_DRAFTS.md`); documentation explicitly foregrounds GDPR data-minimisation, Schrems-II, and EU AI Act alignment. |
| Local-LLM hallucination produces misleading tags | Tags are sidecar metadata, never overwrite source files; schema includes provenance (`model`, `model_hash`, `temperature`, `generated_at`); a `low_confidence` flag is set when the LLM's token-level logprob falls below a tunable threshold; documentation pushes downstream apps to surface tags as suggestions, not ground truth. |
| Conformance-suite gaps cause downstream breakage on edge cases (long paths, non-UTF-8 names, sparse files) | M1's `stony-vdfs-conformance` crate is a published library third parties can run against their own backends; CI tests on Linux + macOS + Windows from day one; a property-based fuzzer (proptest) targets path normalisation explicitly. |
| WER regression threshold not achievable on chosen hardware/model combination during M5 testing | The grant deliverable is the *plumbing*, not the model accuracy; M5 ships against a tested matrix of whisper.cpp model sizes (tiny/base/small/medium/large-v3) so end users select the smallest model that meets their accuracy needs; documentation states accuracy is a property of the model, not the library. |

## 14. Why is this work better done now? (≤ 1200 characters)

Three forces converge in 2026. **Technical:** `whisper.cpp` large-v3
(1.5 GB, Q5_K_M) now reaches 8–12 % WER on a 2021 M1 laptop; `llama.cpp`
runs 3 B GGUFs at ~25 tok/s on the same hardware. The on-device pipeline
that was a research demo in 2022 is viable on commodity hardware today.
**Regulatory:** the EU AI Act enters force August 2026; Schrems-II makes
US-cloud transcription legally fragile for EU public bodies handling
personal data. **Competitive:** Microsoft Recall (Nov 2024) and Apple
Intelligence (Oct 2024) shipped on-device-*ish* indexing — but both are
proprietary, OS-locked, and uninspectable. There is a 12–18 month window
before the application-developer mindshare for "local transcription
plumbing" calcifies around one of those closed stacks. NLnet funding
ships an auditable MIT alternative inside that window.

---

## Submission checklist (internal, do not paste)

- [ ] All draft text reviewed by Anthony for tone (no overclaiming, no
      reference to the closed predecessor by name or product).
- [ ] At least one EU LoS confirmed in writing.
- [ ] Codeberg mirror live and synced.
- [ ] GitHub repo public with `v0.1.0-alpha.1` tag.
- [ ] ≥ 10 commits over ≥ 14 days (not a one-shot drop).
- [ ] `cargo test --workspace` passes locally and in CI on at least
      Linux. (macOS / Windows preferred but not mandatory for M1.)
- [ ] LICENSE file present (MIT).
- [ ] CONTRIBUTING.md and CODE_OF_CONDUCT.md present.
- [ ] ROADMAP.md milestones match this draft 1:1.
- [ ] Application submitted **before 31 May 2026 23:00 EDT** (buffer
      against deadline 1 June 12:00 CEST).

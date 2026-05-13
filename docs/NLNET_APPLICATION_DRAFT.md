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
nothing leaves the device. The project is the open-source distillation of an
architecture refined privately across four Canadian SR&ED R&D cycles
(2022–2025); the NLnet grant funds the work of taking that architecture from
internal prototype to a stable v1.0 library that any application or operating
system distribution can adopt.

## 4. Have you been involved with projects or organisations relevant to this project before? (≤ 2500 characters)

Yes. The applicant has spent 2022–2025 building, in private, a production
desktop application (a media-management product for content creators) whose
core engine is the same VFS abstraction now being open-sourced as
`stony-vdfs`. The work was acknowledged across four successive Canadian
SR&ED claims as eligible experimental development: the novel parts are the
cross-platform path-handling layer (POSIX, Win32, FUSE), the streaming
audio-extraction pipeline that re-encodes losslessly into 16 kHz mono PCM
without buffering the full file, and the privacy-preserving tag pipeline
that keeps every embedding local. The proprietary product remains closed,
but the underlying library and pipeline are being rewritten from scratch
in Rust 2024 edition and released under MIT so that no contractual constraint
of the closed product carries over.

The applicant has contributed (smaller, targeted patches) to: `tokio`,
`bytes`, the FFmpeg Rust bindings, and `whisper.cpp` (issue triage). Those
contributions inform the trait design in `stony-vdfs-core`. The applicant
has not previously applied to NLnet. The application is being filed in
parallel with discussions with Codeberg e.V. and Free Software Foundation
Europe to secure letters of support that anchor the project's European
dimension.

## 5. Requested amount (€)

**€45 000** (forty-five thousand euros) over 12 months, milestone-paid in
six tranches of €7 500 each on delivery of M1–M6.

## 6. Explain what the requested budget will be used for (≤ 5000 characters)

The budget funds **one developer (the applicant) at ~20 hours per week
(50 % FTE) for 12 months** — the minimum cadence at which the project can
reach a stable v1.0 release inside the grant window. The hourly rate
implied (€45 000 / 12 months / 80 hours per month ≈ €47/h) is
self-evidently below market for senior Rust systems development; it is
calibrated to NLnet's grant size, not to commercial billing.

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
- **M2** (Month 3–4) — Local-disk backend production-ready, including
  symlinks, extended attributes, the long tail of cross-platform path
  quirks, and ≥ 95 % line coverage. Tag `v0.2.0`.
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
production-quality library that is useful on its own; M5–M6 add the
content-understanding layer as an opt-in crate. This phasing protects
NLnet against the risk that the AI tooling chain (FFmpeg, whisper.cpp,
llama.cpp) introduces friction late in the grant; the core deliverable
is independent of it.

## 7. Does the project receive (other) funding? (≤ 2500 characters)

No active matching grants. The applicant has historically received
Canadian SR&ED tax credits for the **closed-source** predecessor product;
those credits are accrued, audited, and unrelated to NLnet funding. The
applicant has reviewed the potential interaction between an NLnet grant
and future SR&ED claims with a Canadian tax accountant and confirmed
that the open-source v1.0 work funded by NLnet would be reported
separately and would not double-claim.

## 8. Compare your own project with existing or historical efforts (≤ 2500 characters)

Three layers of comparison:

**Cross-platform VFS abstractions in Rust:** `vfs` (msr-cogito) provides a
synchronous trait surface; `rust-fuse` and `fuser` are Linux/macOS-only
crates that do not abstract over Windows; `winfsp-rs` is the symmetric
Windows-only crate. Existing options force application authors to write
three separate adapters and a synchronisation layer. `stony-vdfs` is the
first Rust workspace that ships an async-first cross-platform VFS
trait with first-class FUSE *and* WinFsp implementations under a single
permissive licence.

**Transcription:** OpenAI Whisper API, Otter.ai, Descript, Microsoft Teams,
and the new Apple/Google on-device transcription stacks all index media
either fully cloud-side or behind a closed runtime. `whisper.cpp` is the
free runtime the field has settled on for local inference; what is missing
is **a library that wires it to a filesystem abstraction** so that
applications can ship Drive-style "search inside your recordings" without
ever leaving the device. That gap is what `stony-vdfs-index` fills.

**Local LLM auto-tagging:** the closest open-source comparator is
`llamafile` + custom plumbing. The novelty is not the LLM call itself
but the integration: tags persist as sidecar JSON in the VFS, are
queryable through the same async API as the files, and survive
backend switches (memory → local → FUSE) unchanged.

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

The closest single-project analogue is the **NGI-funded "Open Virtual
File System" project**, which addresses container-image filesystem
performance rather than user-facing content understanding. `stony-vdfs`
is complementary, not competitive.

## 10. Explain how this project advances the state-of-the-art (≤ 2500 characters)

Three concrete advances:

1. **First permissive-licensed async Rust VFS that compiles for FUSE and
   WinFsp from one workspace.** Today, application authors choose one
   platform and write the others by hand. `stony-vdfs` reduces that to a
   `Cargo.toml` toggle.

2. **First library that closes the loop between FFmpeg, whisper.cpp, and a
   filesystem surface.** Every existing combination requires the developer
   to glue subprocess management, audio re-encoding, model loading, and
   sidecar persistence themselves. The MIT-licensed glue is the novelty —
   nothing else like it ships as a single embeddable crate today.

3. **Architecture that keeps the privacy contract auditable in
   ~3 000 lines of Rust.** Cloud transcription services demand trust in
   tens of millions of lines of closed server code. `stony-vdfs-index` is
   small enough that a downstream auditor can read the entire data-flow
   path in an afternoon and verify that no audio bytes leave the host.

## 11. The team (≤ 2500 characters)

**Anthony Paquet — sole maintainer.** Senior Rust / Tauri / FFmpeg
developer based in Quebec, Canada. ~10 years of professional software
engineering; 4 years of dedicated R&D on the VFS / transcription stack
that underlies this project. The closed-source predecessor product has
been refined across four Canadian SR&ED claims, which require an
auditable record of experimental development — the same engineering
discipline applies to the open-source rewrite.

A small advisory circle (Codeberg e.V., FSFE — pending confirmation; see
`docs/EU_COAPPLICANTS.md`) is being formed to give the project visible
European technical anchorage and to host the EU mirror.

## 12. Do you have any (significant) European linkages? (≤ 1200 characters)

The applicant is Canadian, not EU-resident. To address the European
dimension that NLnet's funding context expects, the project commits to:

1. **Codeberg mirror as a release target** — every tagged release will
   publish to `codeberg.org/stonyp90/stony-vdfs` in parallel with GitHub.
   Forgejo Actions CI is part of M1's CI scope.
2. **Letter of Support from Codeberg e.V.** (Berlin, Germany) — outreach
   underway as of May 2026; LoS template in `docs/EU_COAPPLICANTS.md`.
3. **Letter of Support from FSFE** (Berlin/Hamburg) — outreach underway,
   framed around Public Money? Public Code! alignment for public-sector
   users of on-device transcription.
4. **EU policy alignment narrative**: stony-vdfs implements GDPR's data
   minimisation principle by construction (no data leaves the device);
   it is positioned as infrastructure that EU public bodies can deploy
   without triggering Schrems-II / US CLOUD Act concerns. The
   documentation includes a "Deployment for EU public bodies" guide as
   part of M6.

## 13. Risks and mitigation (≤ 2500 characters)

| Risk                                       | Mitigation                                                                                       |
|--------------------------------------------|--------------------------------------------------------------------------------------------------|
| Solo maintainer becomes unavailable        | Code is permissively licensed, well-documented, and trait-driven; any Rust developer can fork.   |
| FFmpeg / whisper.cpp upstream regressions  | M5–M6 pin specific commits; bring-your-own-binary is supported.                                  |
| WinFsp licence ambiguity for users         | Adapter is feature-gated; users on Windows opt-in explicitly, downstream packagers can omit.     |
| AI tooling chain delays M5–M6              | VFS-first phasing: M1–M4 stand alone as a useful library if M5–M6 slip.                          |
| European dimension review filter           | Codeberg mirror + active LoS outreach; project documentation foregrounds GDPR / sovereignty fit. |

## 14. Why is this work better done now? (≤ 1200 characters)

Local-first AI passed the usable-on-commodity-hardware threshold in 2024
(`whisper.cpp` small models, `llama.cpp` 1–3 B quantised GGUFs). Before
that threshold, on-device transcription was a research curiosity; today
it is a viable replacement for the SaaS pipeline. There is a 12–24 month
window in which the libraries to embed it cleanly will be written by
*someone*; if that someone is a US BigTech wrapper, the privacy contract
will be optional and quietly violated. If it is a permissively licensed
European-anchored project, the privacy contract is structural. NLnet
funding makes the second outcome possible inside the window.

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

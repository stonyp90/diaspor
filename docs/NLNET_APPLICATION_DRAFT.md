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

**Cross-platform VFS abstractions in Rust:** `vfs` (msr-cogito,
synchronous), `virtual-fs` (wasix, WASM-flavoured), `opendal` (Apache,
object-store first, no FUSE/WinFsp surface), `fuser` (Linux/macOS only,
no Windows), `polyfuse` (async but Linux-only), `winfsp-rs` and
`winfsp-sys` (Windows only). C/C++ historical comparators: libcfu, GVfs
(GNOME, GPL, server-tied), Dokany (Windows, fragmented governance).
None ship a single async Rust workspace with first-class FUSE *and*
WinFsp implementations under one permissive licence. Existing options
force authors to write 2–3 adapters plus a sync layer.

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
engineering across systems programming, desktop applications, and media
pipelines; 4 years (2022–2025) of dedicated R&D on the VFS /
transcription stack that underlies this project, documented across four
successive Canadian SR&ED claims acknowledged by the Canada Revenue
Agency as eligible experimental development under section 248(1) ITA.
Each SR&ED claim is a written, auditable technical-uncertainty narrative
reviewed by CRA's scientific advisors; the cumulative R&D effort
captured in those four claims is what `stony-vdfs` distils into open
source.

Verifiable artefacts NLnet reviewers can check independently:

- **GitHub profile:** github.com/stonyp90 — public commit history,
  Rust contributions to upstream crates (`tokio`, `bytes`, FFmpeg-Rust
  bindings, `whisper.cpp` issue triage), and the `stony-vdfs` repository
  itself with ≥ 10 commits over ≥ 14 days prior to submission.
- **Closed-source predecessor product:** four years of production use
  with paying customers, processing terabytes of media through the same
  FFmpeg/Whisper/local-LLM pipeline now being re-implemented openly in
  Rust 2024 edition under MIT.
- **Cross-platform shipping experience:** the predecessor product ships
  on Linux, macOS, and Windows simultaneously — the cross-platform
  pain that `stony-vdfs` solves is pain the applicant has paid four
  years of debugging time on.
- **Canadian SR&ED documentation:** available in redacted form on
  request to NLnet's review panel under NDA (the SR&ED narratives
  themselves are confidential to CRA, but the existence and acceptance
  of the four claims is verifiable through Canadian government tax
  records).

A small advisory circle is being formed: outreach in progress with
Codeberg e.V. (Berlin), FSFE (Berlin/Hamburg), Linagora/LinTO (Paris),
KDE e.V. (Berlin), and Funkwhale (NLnet alumnus, Germany) — see
`docs/EU_COAPPLICANTS.md` for the full plan, contact verification, and
14-day timeline. The goal is at least one signed Letter of Support
from an EU-27 organisation before submission, ideally two, to give the
project visible European technical anchorage and to host the EU mirror.

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
| Solo maintainer becomes unavailable (illness, life event) | Trait-driven architecture, MIT licence, public roadmap, and milestone-gated commits mean any Rust developer can fork at any tagged release; NLnet's milestone-based disbursement also caps grant exposure to the last completed milestone, protecting NLnet from sunk-cost loss. |
| FFmpeg / whisper.cpp / llama.cpp upstream regressions | All three dependencies are subprocess- or FFI-wrapped, never statically linked into the core; M5–M6 pin specific commits in `Cargo.lock` and document a tested binary matrix; bring-your-own-binary is the default. |
| WinFsp licence ambiguity (GPLv3 + commercial dual-licence) | The `stony-vdfs-winfsp` crate is feature-gated and isolated in its own workspace member; downstream MIT-only packagers can omit it entirely and still ship FUSE + memory + local backends. |
| AI tooling chain (whisper.cpp/llama.cpp APIs) shifts mid-grant | VFS-first phasing: M1–M4 deliver a stand-alone production library worth €30 000 of value if M5–M6 must be re-scoped. Trait surface for index pipeline is decoupled from any specific runtime. |
| European-dimension review filter rejects non-EU solo applicant | Codeberg mirror committed in M1; active LoS outreach to Codeberg e.V., FSFE, Linagora/LinTO, KDE e.V., Funkwhale (NLnet alumnus) per `docs/EU_COAPPLICANTS.md`; documentation explicitly foregrounds GDPR data-minimisation, Schrems-II, and EU AI Act alignment. |
| Local-LLM hallucination produces misleading tags | Tags are sidecar metadata, never overwrite source files; schema includes provenance (`model`, `model_hash`, `temperature`, `generated_at`); a `low_confidence` flag is set when the LLM's token-level logprob falls below a tunable threshold; documentation pushes downstream apps to surface tags as suggestions, not ground truth. |
| Conformance-suite gaps cause downstream breakage on edge cases (long paths, non-UTF-8 names, sparse files) | M1's `stony-vdfs-conformance` crate is a published library third parties can run against their own backends; CI tests on Linux + macOS + Windows from day one; a property-based fuzzer (proptest) targets path normalisation explicitly. |

## 14. Why is this work better done now? (≤ 1200 characters)

Three forces converge in 2026. **Technical:** `whisper.cpp` large-v3
(1.5 GB, Q5_K_M) now hits 8–12 % WER on a 2021 M1 laptop; `llama.cpp`
runs 3 B GGUFs at 25 tok/s on the same hardware. The on-device pipeline
that was a research demo in 2022 is production-grade today. **Regulatory:**
the EU AI Act enters force August 2026; GDPR fines hit €1.2 B in 2024
alone; Schrems-II makes US-cloud transcription legally fragile for EU
public bodies. **Competitive:** Microsoft Recall (Nov 2024) and Apple
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

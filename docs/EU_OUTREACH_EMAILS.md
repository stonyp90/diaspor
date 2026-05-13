# EU Outreach Emails — Letter-of-Support Requests

**Project:** `stony-vdfs` — Rust virtual filesystem with on-device FFmpeg transcription and local-LLM auto-tagging (MIT)
**Sender:** Anthony Paquet (`anthonypaquet1508@gmail.com`), Quebec, Canada
**Purpose:** NLnet NGI Zero Commons Fund, 13th call, deadline 2026-06-01 12:00 CEST
**Send date target:** Week of 2026-05-13

Three fully personalised emails follow, one per priority candidate. Each is intended to be pasted into a plain-text email client (no HTML) and sent from the applicant's primary address. Subject lines are included.

---

## Candidate 1 — Codeberg e.V.

**To:** `codeberg@codeberg.org` (governance / membership) — primary
**CC:** `help@codeberg.org` (general fallback)
**Subject:** Mirror commitment + letter-of-support request from a Rust local-first project (NLnet, June 1)

Hi Codeberg team,

I'm Anthony Paquet, a Canadian Rust developer preparing an NLnet NGI Zero Commons Fund application due 1 June for **stony-vdfs** — an MIT-licensed virtual filesystem in Rust that does privacy-first audio/video transcription via FFmpeg + whisper.cpp and auto-tagging via a small local LLM. No bytes leave the user's machine. Repo: github.com/stonyp90/stony-vdfs.

The reason I'm writing you specifically: Codeberg, and Forgejo, are exactly the kind of EU-sovereign infrastructure that the project's pitch leans on. I want stony-vdfs to be visibly anchored on Codeberg, not just GitHub. Concretely, I'm committing to (a) a synced mirror at `codeberg.org/stonyp90/stony-vdfs` from `v0.1.0` onward, (b) Forgejo Actions CI as part of M1's scope so every tagged release is reproducibly built on Codeberg infrastructure, and (c) upstream issue reports against Forgejo when I hit edges during integration — small but real signal back to the cooperative.

NLnet weights the "European dimension" criterion heavily, and a Canadian solo applicant without a single EU letter of support is typically filtered in the first review pass. The ask is small: **would Codeberg e.V. write a short (one-paragraph) letter of support** confirming that the project's mirror commitment and Public Money? Public Code! values alignment are real, and that Codeberg is willing to host the mirror? I'm happy to draft a version for the board to edit so this stays a 10-minute task on your side.

Glad to share the application draft or jump on a 20-minute call any time before 27 May.

Thanks for considering — and for running Codeberg in the first place.

Anthony Paquet
github.com/stonyp90 · `anthonypaquet1508@gmail.com`
README: github.com/stonyp90/stony-vdfs#readme

---

## Candidate 2 — Free Software Foundation Europe (FSFE)

**To:** `contact@fsfe.org`
**CC:** (none — let FSFE route internally; PMPC team is reachable via `contact@`)
**Subject:** Public Money? Public Code! aligned local-first transcription stack — letter of support for NLnet (June 1)

Hello FSFE team,

I'm Anthony Paquet, a Canadian Rust developer writing because the project I'm filing with NLnet next month is a direct operationalisation of Public Money? Public Code! for one specific, painful use case: **public-sector audio and video transcription** that today depends almost entirely on US SaaS (Microsoft Teams, Otter, Descript, Rev, AWS Transcribe). Parliaments, courts, public broadcasters, and journalists upload sensitive audio to US-hosted services every working day. Schrems-II makes that a legal liability; the EU AI Act, in force August 2026, makes it a compliance liability too.

**stony-vdfs** is the underlying infrastructure: an MIT-licensed Rust virtual filesystem (memory / local / FUSE / WinFsp) with an opt-in indexing pipeline running FFmpeg + whisper.cpp + a small local LLM **entirely on-device**. No telemetry, no cloud calls, no API keys — the privacy contract is structural. Repo: github.com/stonyp90/stony-vdfs. The 12-month roadmap commits a "Deployment for EU public bodies" guide in M6 that maps explicitly onto the PMPC criteria.

NLnet typically filters non-EU solo applicants in the first pass without a European letter of support. **Would FSFE write a short LoS** confirming the design is PMPC-compliant in spirit and that the public-sector framing is credible? Even 5–7 sentences from FSFE carries enormous weight with NLnet reviewers. I'm not asking for co-applicant status or any financial role — just a written endorsement of values alignment.

Happy to share the draft or write a candidate paragraph FSFE can edit. Deadline is 1 June; anything by 27 May would be perfect.

Thanks for the work on PMPC — it's the policy frame this project was built inside.

Anthony Paquet
github.com/stonyp90 · `anthonypaquet1508@gmail.com`

---

## Candidate 3 — Linagora / LinTO

**To:** `hello@linto.ai` (LinTO team) — primary
**CC:** Linagora contact form at `linagora.com/en/contact` (secondary; mirror the email body there)
**Subject:** stony-vdfs × LinTO — complementary local-first transcription stack, letter-of-support ask for NLnet

Bonjour LinTO team,

I'm Anthony Paquet, a Quebec-based Rust developer (French-speaking, writing in English so the wider team can skim). I'm preparing an NLnet NGI Zero Commons Fund application due 1 June for **stony-vdfs** — an MIT-licensed virtual filesystem in Rust whose opt-in indexing layer wraps FFmpeg + whisper.cpp + a small local LLM to give downstream applications "Drive-style search across your media, no cloud calls." Repo: github.com/stonyp90/stony-vdfs.

I've been following LinTO since the European Commission / European Parliament deployments became public — your six-year run of pre-Whisper sovereign-AI R&D is the bar this whole space should be measured against, and LUCIE is the LLM I'd most like to make a first-class tagger backend for in M6.

The reason I'm writing rather than just shipping a quiet competitor: **stony-vdfs is complementary, not competitive.** LinTO is a server product with institutional deployments; stony-vdfs is a filesystem-native, embeddable Rust library for individual application developers (desktop, mobile, edge). The use cases barely overlap. The integration story is clean — LinTO Studio can ingest stony-vdfs sidecar JSON, stony-vdfs can target LinTO endpoints as one of several pluggable backends. I'd happily commit a "stony-vdfs × LinTO Studio" integration cookbook as a public grant deliverable.

The concrete ask: **would Linagora write a short letter of support** confirming the complementarity and your willingness to host the cookbook on linagora.ai? NLnet flags non-EU solo applicants without an EU LoS, and one from LinTO would land with particular credibility.

Happy to share the application draft or hop on a 30-minute call in French or English. Anything by 27 May would be a gift.

Merci d'avance,
Anthony Paquet
github.com/stonyp90 · `anthonypaquet1508@gmail.com`

---

## Send-tracking checklist

- [ ] Codeberg sent (target Wed 14 May)
- [ ] FSFE sent (target Thu 15 May)
- [ ] Linagora/LinTO sent (target Thu 15 May)
- [ ] First follow-up Tue 20 May (if no reply by then)
- [ ] LoS PDFs filed in `docs/los/` once received
- [ ] EU_COAPPLICANTS.md updated with response status per candidate

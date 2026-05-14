# EU Co-Applicants & Letter of Support Candidates

**Project:** `cairn` — Rust virtual filesystem with privacy-first FFmpeg + whisper.cpp transcription + local LLM auto-tagging (MIT, github.com/stonyp90/cairn)
**Funding target:** NLnet NGI Zero Commons Fund, 13th call, deadline **2026-06-01 12:00 CEST**
**Document purpose:** Identify 5+ EU-based individuals/organizations who can co-apply or provide a letter of support to satisfy NLnet's "European dimension" criterion, which is effectively a knockout filter for Canadian solo applicants.
**Status:** Research-stage shortlist. Contact info verified via public pages as of May 2026.

---

## Why this matters

NLnet's Commons Fund is EU-funded (Horizon Europe / DG CNECT). Reviewers explicitly weight projects on European policy alignment, sovereignty over data and AI, and demonstrable EU community involvement. A Canadian solo applicant with no EU letter of support is at extreme risk of being filtered in the first administrative review pass — independent of project merit. One credible EU letter of support (LoS) typically clears the bar; a co-applicant arrangement is stronger but introduces budget-sharing complexity. The goal of the next 14 days is to land **at least one signed LoS**, ideally two, from organizations on this list.

---

## Candidates (ranked by strategic value)

### 1. Codeberg e.V. — Berlin, Germany

- **Country:** Germany (Berlin)
- **Type:** Registered non-profit association (eingetragener Verein), FOSS Git hosting cooperative, ~7 founding members, launched 2019
- **Why a good fit:** Codeberg hosts non-Microsoft, EU-sovereign infrastructure for open-source code — a direct mission overlap with cairn' "privacy-first, local-first" pitch. Moving the project's mirror to Codeberg as part of the grant deliverables gives them a concrete reason to write a support letter. They explicitly back projects that strengthen European digital sovereignty. Forgejo, which Codeberg maintains, has itself been NGI-funded — they understand the funding context.
- **Contact info (verified):**
  - General: `help@codeberg.org`
  - Membership / governance: `codeberg@codeberg.org`
  - Press: `press@codeberg.org`
  - Site: https://codeberg.org/Codeberg-e.V./
- **Approach angle:** Lead with "we will mirror to Codeberg, integrate Forgejo Actions for CI, and contribute upstream issue reports during the grant period." Cite Public Money? Public Code! alignment. Ask for a short LoS confirming hosting commitment and FOSS values alignment — easy ask, low friction.
- **Likelihood:** **HIGH** — small, mission-aligned, used to receiving LoS requests. Best first contact.

---

### 2. Free Software Foundation Europe (FSFE) — Berlin/Hamburg, Germany

- **Country:** Germany (legal seat Hamburg; primary office Berlin, Revaler Straße 19, 10245)
- **Type:** Registered voluntary association under German law; sister org to FSF/US; runs the "Public Money? Public Code!" campaign and is an associate of KDE e.V.
- **Why a good fit:** cairn is MIT-licensed, public-money-eligible (research credits) software, and addresses a sovereignty concern (no cloud transcription). FSFE's PMPC campaign explicitly targets the kind of public-sector use case where a sovereign local-first transcription stack is preferable to Otter/Descript/Microsoft Teams. They have a long history of endorsing NGI Zero applicants with policy-aligned missions.
- **Contact info (verified):**
  - General: `contact@fsfe.org`
  - Privacy / DPO: `privacy@fsfe.org`
  - Contact page: https://fsfe.org/about/contact.en.html
  - President: Matthias Kirschner
- **Approach angle:** Frame cairn as PMPC-aligned tooling for public broadcasters, parliaments, courts, and journalists who currently depend on US SaaS transcription. Mention you'll add a "PMPC-compliant" deployment guide. Ask for an LoS, not co-applicant status (FSFE generally avoids financial co-applicant roles).
- **Likelihood:** **MEDIUM-HIGH** — FSFE writes a lot of LoS but vets carefully. Worth a personalized email referencing specific PMPC documents.

---

### 3. Linagora / LinTO — Paris & Issy-les-Moulineaux, France

- **Country:** France
- **Type:** French open-source software publisher (SAS); LinTO is its sovereign open-source AI transcription product line, used by the European Commission and European Parliament
- **Why a good fit:** This is the single closest technical overlap on the list. LinTO does exactly what cairn' transcription module does — Whisper-based audio transcription, live subtitling, summarization — but with an institutional focus. They have six years of pre-Whisper R&D in this space and have publicly positioned themselves around European AI sovereignty. cairn as a filesystem-native, local-first complement to LinTO's server product is a clean differentiation story; they don't compete.
- **Contact info (verified):**
  - LinTO team: `hello@linto.ai`
  - Linagora main: contact form at https://linagora.com/en/contact
  - GitHub org: https://github.com/linto-ai
  - Sites: https://linagora.ai/en, https://linto.ai
- **Approach angle:** Position as complementary, not competitive. "Your customers can pipe cairn output into LinTO Studio." Offer to publish a joint integration cookbook as a grant deliverable. Mention LUCIE LLM compatibility for the tagging module. This is the strongest co-applicant candidate on the list because they have institutional EU AI experience.
- **Likelihood:** **MEDIUM** — they're commercial, so an LoS is more likely than a true co-applicant arrangement. Initial ping should be exploratory.

---

### 4. Martin Kleppmann — University of Cambridge, UK & TU Munich alumnus

- **Country:** UK (Cambridge) with deep TU Munich (Germany) ties
- **Note on UK status:** Post-Brexit, UK is associated to Horizon Europe as of 2024, and UK researchers can serve on NLnet applications. UK-affiliated LoS still counts for European dimension purposes in practice.
- **Type:** Associate Professor in Computer Security and Privacy, Department of Computer Science and Technology, University of Cambridge; previously Research Fellow at TU Munich (2022–2023); author of "Designing Data-Intensive Applications" and co-author of the original 2019 local-first manifesto
- **Why a good fit:** Kleppmann co-coined "local-first software" — the exact philosophical frame cairn uses. An LoS from him would be the single most credible academic endorsement for the project's positioning. His research group at Cambridge studies distributed, end-to-end-encrypted local-first systems (Automerge, etc.), which is technically adjacent to a virtual filesystem with local AI processing.
- **Contact info (verified):**
  - University page: https://www.cst.cam.ac.uk/people/mk428 (Cambridge directory lists his contact)
  - Personal site: https://martin.kleppmann.com/
  - Affiliation: William Gates Building, 15 JJ Thomson Avenue, Cambridge CB3 0FD
- **Approach angle:** Reference the 2019 manifesto explicitly. Ask not for funding endorsement broadly but for a paragraph confirming that cairn' architecture instantiates principles from his manifesto (long-term data ownership, working offline, no lock-in). Short, intellectual ask. Offer to credit/cite his work prominently in the README and docs.
- **Likelihood:** **MEDIUM** — academics get LoS requests constantly but ones that genuinely engage with their published work get responses. Write the most personalized email of any candidate to him.

---

### 5. KDE e.V. — Berlin, Germany

- **Country:** Germany (Berlin, Prinzenstraße 85 F, 10969)
- **Type:** Non-profit association, supports the KDE community; founded 1997; associate of FSFE; maintains Subtitle Composer and other media-handling FOSS tools
- **Why a good fit:** KDE maintains Subtitle Composer, Kdenlive (video editor), and other media tools that could integrate with cairn as a transcription/tagging backend. Many KDE applications could benefit from a Linux-native virtual filesystem that exposes auto-transcribed media. KDE e.V. has supported NGI Zero applicants with LoS in past rounds and is broadly aligned with FOSS sovereignty messaging.
- **Contact info (verified):**
  - Board: `kde-ev-board@kde.org`
  - General contact: https://ev.kde.org/contact/
  - Phone: +49 30 2023 7305-0
- **Approach angle:** Propose integration with Kdenlive (auto-subtitles via cairn) and/or Subtitle Composer as a grant milestone. KDE e.V. responds well to concrete integration commitments rather than abstract endorsements.
- **Likelihood:** **MEDIUM** — board needs to approve, so allow ~10 days response time. Less personal than other targets but very legitimate.

---

### 6. Framasoft — Lyon, France

- **Country:** France (Lyon, 69007)
- **Type:** Association loi 1901, ~35 members including 9 employees; runs the de-google-ify-internet campaign; operates Framaforms, PeerTube ecosystem support, hosts many federated FOSS tools
- **Why a good fit:** Framasoft is the highest-profile French FOSS advocacy nonprofit; they care about local data ownership, alternatives to US SaaS, and have funded/promoted Peertube which itself has transcription/subtitling needs. A virtual filesystem that lets PeerTube instances auto-transcribe uploads locally is a credible integration story. Their reach into the French-speaking FOSS world adds a non-Anglophone EU voice to the application — explicit value-add for NLnet reviewers.
- **Contact info (verified):**
  - Main: `contact@framasoft.org`
  - Contact form: https://contact.framasoft.org/en/
- **Approach angle:** Pitch the PeerTube integration possibility. Acknowledge the language angle explicitly — "we want a French-speaking EU partner so this isn't just an Anglo project." Their contact form routes by topic; pick "Partnerships" or "Press contact."
- **Likelihood:** **MEDIUM-LOW** — they're small, busy, and don't reply to every inquiry. But a strong, specific email could land.

---

### 7. Funkwhale community / Georg Krause (maintainer) — Germany

- **Country:** Germany (lead maintainer)
- **Type:** FOSS decentralized audio platform, ActivityPub-based, **already an NLnet/NGI Zero grant recipient** (Discovery Fund and follow-on rounds)
- **Why a good fit:** Audio platform with podcast/music-server use cases — transcription of episodes is a long-standing wishlist feature. An NLnet alumnus's LoS carries unusual weight with NLnet reviewers because they vouch from inside the funded community. The community has openly looked for new maintainers, indicating openness to collaborators.
- **Contact info (verified):**
  - Project hub: https://funkwhale.audio
  - Governance forum: https://governance.funkwhale.audio (Georg posts as a maintainer)
  - Blog: https://blog.funkwhale.audio
  - NLnet project page: https://nlnet.nl/project/Funkwhale/
- **Approach angle:** Reach out via governance forum (public, low friction) before a private email. Propose a "cairn as Funkwhale transcription backend" prototype as a stretch goal. Mention you've read their NLnet milestone reports — small detail, big signal.
- **Likelihood:** **MEDIUM** — small team, but NLnet-alumnus LoS is high-value. Best ROI per hour spent.

---

## Outreach plan — 14-day timeline (May 13–27, 2026)

| Day | Date | Action |
|-----|------|--------|
| 0 | Tue May 13 | Finalize this shortlist; draft the personalized opening paragraph for each of the top 4 (Codeberg, FSFE, Linagora, Kleppmann) |
| 1 | Wed May 14 | Send emails 1 & 2: **Codeberg** (`help@codeberg.org`) and **Kleppmann** (Cambridge address) — highest-likelihood and longest-lead-time targets first |
| 2 | Thu May 15 | Send emails 3 & 4: **FSFE** (`contact@fsfe.org`) and **Linagora/LinTO** (`hello@linto.ai`) |
| 3 | Fri May 16 | Post on **Funkwhale governance forum** introducing project; send email to **KDE e.V. board** (`kde-ev-board@kde.org`) |
| 5 | Sun May 18 | Send **Framasoft** contact-form message (lower urgency) |
| 7 | Tue May 20 | First follow-up to non-responders (Codeberg, Kleppmann) — short, polite, one-paragraph |
| 9 | Thu May 22 | Begin drafting the actual NLnet application with whichever LoS commitments are in hand |
| 10 | Fri May 23 | Follow-up to second wave (FSFE, Linagora, KDE) |
| 12 | Sun May 25 | Final follow-up to any silent candidates; if ≥1 LoS confirmed, lock in scope. If 0, escalate to backup candidates (Public Code, Prav cooperative, individual NGI Zero alumni — research list TBD) |
| 14 | Tue May 27 | All LoS finalized as PDFs; integrate into application package |
| — | Wed May 28 – Sat May 31 | Final application polish; budget refinement; submission rehearsal |
| — | Mon Jun 1, 12:00 CEST | **SUBMIT** |

**Critical-path rule:** if by Day 9 (May 22) zero LoS commitments are confirmed in writing, escalate to backup tier and consider whether to defer submission to the next NLnet call (roughly Aug 2026). A non-EU solo application without LoS is a wasted submission slot.

---

## Template outreach email (~150 words)

Subject: **Letter of support request — local-first transcription filesystem (NLnet NGI Zero, due June 1)**

> Hi [Name / Team],
>
> I'm Anthony Paquet, a Canadian Rust developer. I'm preparing an NLnet NGI Zero Commons Fund application (deadline June 1) for **cairn** — an MIT-licensed virtual filesystem in Rust that does privacy-first audio/video transcription via whisper.cpp and local LLM auto-tagging. No data leaves the user's machine. Repository: github.com/stonyp90/cairn.
>
> NLnet weights "European dimension" heavily. I'm reaching out because [one specific, personalized sentence — e.g. "Codeberg's Forgejo CI is on my milestone list" / "your 2019 local-first manifesto is the architectural frame for this project" / "LinTO and cairn are complementary — filesystem-native vs server"].
>
> Would you be open to a short (one-paragraph) **letter of support** confirming the alignment? Happy to draft a version for you to edit. Glad to share the application draft if useful. I can answer questions on a 20-min call any time this week.
>
> Thanks for considering,
> Anthony Paquet
> [email] · [GitHub] · [link to README]

**Customization checklist per candidate:**
- Codeberg → mention Forgejo CI integration + project mirror commitment
- FSFE → cite Public Money? Public Code! and a specific public-sector use case (parliament transcription, court records)
- Linagora → cite LinTO's six-year history, propose joint integration cookbook
- Kleppmann → cite the 2019 manifesto authors by name, point to specific README sections that operationalize the principles
- KDE e.V. → name Kdenlive / Subtitle Composer integration milestones
- Framasoft → write in French if you can; mention PeerTube integration angle
- Funkwhale → reference their NLnet project page and milestone reports; propose podcast/music-library transcription

---

## Backup candidates (if first-tier yields zero by Day 9)

These are lower-effort screens and not yet fully researched, but plausible:

- **Prav cooperative** (privacy messenger, Czech-ish, in registration as cooperative) — `https://prav.app`
- **NLnet itself** for an informal "is this in-scope?" pre-submission check via the application Q&A inbox (does not count as LoS but de-risks scope mismatch)
- **Public Code / publiccode.eu** team — adjacent to FSFE PMPC
- **WhisperLive maintainers at Collabora** (UK-headquartered open-source consultancy with EU presence) — `https://collabora.com`
- **Individual NGI Zero alumni** from the 2024–2026 batches whose projects touch FFmpeg, FUSE, or local AI — searchable on `nlnet.nl/project/`

---

## Risks and caveats

- **No fabricated contacts.** Every email above is sourced from the organization's published contact page or directory and verified at the time of research. Re-check before sending, as small orgs change addresses.
- **LoS does not guarantee co-funding.** An LoS is a written statement of alignment; a co-applicant is a budget-sharing partner. NLnet accepts both, but LoS is far easier to secure in a 14-day window.
- **Brexit caveat.** UK-affiliated researchers (Kleppmann) count under Horizon Europe association, but if NLnet's reviewers are conservative, pair Kleppmann with a clearly EU-27 LoS (Codeberg or FSFE).
- **Tone.** EU FOSS orgs are allergic to startup-speak. Drop all "disrupt," "revolutionize," "AI-powered" language. Write like a maintainer, not a founder.
- **Language.** Anthony writing in French to Framasoft is a meaningful signal. English elsewhere is fine.

---

*Document prepared 2026-05-13. Re-verify contact endpoints before sending. Treat as a working file; check off candidates as outreach completes.*

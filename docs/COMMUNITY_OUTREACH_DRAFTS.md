# Community Outreach Drafts — cairn

**Purpose:** seed early visibility, attract one or two real non-author contributors before May 22, and document EU-friendly outreach for the NLnet NGI Zero Commons Fund application (deadline June 1, 2026).

**Tone rules for every piece below:**
- Write like a tired senior dev, not a marketing team.
- No "delve", "leverage", "ecosystem", "robust", "unlock", "supercharge".
- Concrete over abstract. Code over claims.
- Admit what does not work yet. People trust that more than a polished pitch.

---

## A. Mastodon launch post (500 chars max)

> Just put `cairn` up on GitHub: a Rust virtual filesystem with a hook for local FFmpeg + whisper.cpp transcription and on-device LLM tagging. No cloud, no telemetry, no API keys.
>
> Memory + local backends work today. FUSE/WinFsp/index pipeline land over the next 12 months (NLnet NGI Zero application in flight).
>
> Looking for early eyes on the trait surface before I freeze it.
>
> https://github.com/stonyp90/cairn
>
> MIT. Rust 2024.
>
> #rust #foss #fediverse

(Character count: 478. Includes trailing newline; within Mastodon's 500-char default.)

---

## B. r/rust "show-and-tell" post (300-400 words)

**Title:** A virtual filesystem trait that I also want to hang a local whisper.cpp pipeline off — sanity check before I freeze the API?

**Body:**

I've been writing a Rust workspace called `cairn` for a few months and just pushed it to GitHub. Looking for feedback before M1 lands and the public traits get locked.

The shape of it:

- `cairn-core` defines `VfsBackend` and `VfsHandle` as async traits. Pure traits, no IO deps, no tokio in the core crate (callers pick the runtime).
- Two backends today: in-memory (passes the conformance suite) and local POSIX/Windows (happy path; symlinks + xattrs land in M2).
- FUSE adapter (`fuser`) and WinFsp adapter are scaffolded with a trait surface only. Real implementations in M3/M4.
- A separate optional crate, `cairn-index`, decorates any backend and runs a content pipeline: FFmpeg subprocess to probe and extract audio, whisper.cpp via `whisper-rs` for transcription, a local LLM (ollama/llama.cpp) for tagging, sidecar JSON written back through the VFS. That whole pipeline is M5-M6; the traits are public now so people can design against them.

Things I would genuinely like a Rust-flavoured opinion on:

1. The async-trait surface. I am using native `async fn` in trait (2024 edition), not the `async-trait` macro. Curious whether that hurts anyone trying to box `dyn VfsBackend`. There is an object-safe wrapper in `core::dyn_backend` but I am not in love with it.
2. The way the indexing layer wraps a backend as a decorator (`IndexingBackend<B>`) rather than living as a side service. It keeps the public API tiny but means everything goes through one trait. Fine? Wrong?
3. Conformance test crate (`cairn-conformance`) — `conformance::run(&backend).await`. Any third-party backend can plug in. I have not seen this exact pattern in the Rust filesystem-trait space; if there is prior art I should copy from, please point me at it.

What works today: `cargo test --workspace` is green on Linux/macOS/Windows in CI. What does not: FUSE mount, WinFsp mount, the indexing pipeline. Those are the next twelve months of work.

MIT, Rust 1.85+. https://github.com/stonyp90/cairn

Happy to take a one-line typo PR or an issue if anything jumps out as weird.

---

## C. r/selfhosted post (250-350 words)

**Title:** Working on a privacy-respecting media indexer (Rust, local-only, FFmpeg + whisper.cpp) — would love use-case feedback

**Body:**

If you self-host a media library — Jellyfin, Navidrome, a Funkwhale or PeerTube instance, a directory of voice memos — you have probably wanted a "find that podcast where they mentioned the new router firmware" search box. The big cloud providers do this server-side: they transcribe and index your media in their data centres. Useful. Also a privacy trade-off some of us would rather not make.

I've been building `cairn` as the plumbing piece for the local-only version of that feature. It is a Rust library, MIT-licensed. The pitch:

- Drop media files into a folder (or any backend it knows about).
- The library probes them with FFmpeg, transcribes audio with whisper.cpp, tags the transcript with a small local LLM (llama.cpp or ollama).
- Transcripts and tags are stored as sidecar JSON next to the original files — so you can `grep` them with normal shell tools.
- Zero network calls. Models live on your machine. No API keys.

It is meant to be the index/storage piece — not a full app. Downstream projects (a Jellyfin plugin, a "podcast grep" CLI, a backup tool that indexes media before storing it) would build on top.

Status, honestly: the filesystem layer works today. The whisper.cpp + LLM pipeline is scaffolded but the real implementations land over the next 9-12 months. I am writing it under a NLnet NGI Zero application; whether or not that lands, the code is MIT and the roadmap is public.

What I would love from selfhosters:

- Tell me what you would actually want to search across. Podcasts? Voice notes? Old camcorder footage? Lecture recordings?
- What folder structures and metadata conventions do you already use that I should not break?
- If you mount stuff over FUSE today, what hurts?

Repo: https://github.com/stonyp90/cairn — issues open. No installer yet, this is library-stage.

---

## D. HackerNews "Show HN" post

**Title (60 chars max):** Show HN: Stony-vdfs – Rust VFS with local Whisper transcription

(58 chars including "Show HN: " prefix.)

**First comment (150-200 words):**

Author here. Quick context, since the README is light on "why":

I wanted a single Rust library that gives an application a uniform async filesystem trait (memory, local disk, FUSE, WinFsp) and lets me hang a content-understanding pipeline off it — FFmpeg probe + whisper.cpp transcription + local LLM tagging — without any of it phoning home. The use case is desktop and self-hosted apps that want their users' media to be searchable without uploading it.

What works today: core traits, in-memory backend, local POSIX/Windows backend, conformance test crate. CI green on Linux/macOS/Windows.

What does not work yet: FUSE mount, WinFsp mount, and the full transcription/tagging pipeline. The traits are public; the heavy implementations land in milestones M3-M6 over the next year. I am applying to NLnet NGI Zero for that work; the code stays MIT regardless of the outcome.

The async-fn-in-trait surface and the decorator pattern for indexing are the two things I most want to harden before v1.0. Questions and "this is wrong because…" responses welcome.

---

## E. Personalized DM templates (3)

### E1. Rust ecosystem developer (tokio / async-std / runtime-adjacent)

> Hey [NAME] — long-time user of [SPECIFIC_PROJECT], thanks for that work.
>
> I am putting up a small Rust library called `cairn` — a virtual filesystem trait with async fn in trait (no `async-trait` macro), in-memory + local backends today, FUSE/WinFsp + a local whisper.cpp/LLM indexing pipeline over the next year.
>
> Because you live closer to the async-in-trait edge cases than most people, I would value a five-minute glance at `cairn-core/src/lib.rs` to tell me if I have made anything obviously dumb with the trait object dance in `dyn_backend.rs`. If you spot a typo or an unclear doc comment, a one-line PR or a "this is confusing" issue would be enormously useful — I am trying to seed real community signal before a funding application closes on June 1.
>
> Repo: https://github.com/stonyp90/cairn
>
> Not asking for endorsement. Just a critical eye if you have a minute.
>
> Cheers,
> Anthony

### E2. EU-based FOSS developer (Codeberg / Funkwhale / European Rust)

> Hallo [NAME],
>
> I follow your work on [SPECIFIC_PROJECT] — really appreciate it.
>
> I am a Canadian Rust dev about to make `cairn` public: a small MIT-licensed virtual filesystem with optional on-device transcription (FFmpeg + whisper.cpp) and local-LLM tagging. No cloud calls, no telemetry. The roadmap is going into an NLnet NGI Zero application next week.
>
> The repo is mirrored on Codeberg precisely because I want European visibility on it from day one. I am not asking for a letter of support (different conversation). What would actually help: a five-minute look at the README and the trait surface, and an issue if anything in the docs is wrong or confusing. Even a typo PR would be a useful signal of EU community interest.
>
> Repo: https://github.com/stonyp90/cairn — Codeberg mirror in the README.
>
> Thanks for considering. If now is bad, no worries at all.
>
> Anthony

### E3. Privacy-focused developer (Tor / Tails / Mozilla / Signal contributor)

> Hi [NAME],
>
> Reaching out cold — apologies. I have read [SPECIFIC_PROJECT] (and the surrounding writing) and your framing around [SPECIFIC_TOPIC, e.g. "metadata as adversary"] shaped how I am writing the threat model for a small project I just made public.
>
> The project is `cairn`: a Rust virtual filesystem that, optionally, runs FFmpeg + whisper.cpp + a local LLM over media files to make them searchable, without making a single network call. The whole pitch is "the same UX as the cloud transcription feature, but the audio never leaves the machine."
>
> I am not asking for an endorsement. What would help: if you have ten minutes, look at `docs/PRIVACY.md` (planned for M5 — currently a stub) and the README's "Why does this matter?" section, and open an issue if the threat model feels naive or if I have over-claimed anything. A one-line correction PR is even better.
>
> Repo: https://github.com/stonyp90/cairn
>
> Thanks for reading this far,
> Anthony

---

## F. NLnet alumni network outreach (4 emails)

Each of the targets below is an EU-based individual or organization that has previously received NLnet / NGI Zero funding and has a plausible technical reason to care about a privacy-first VFS with local transcription. The ask is small: a 5-10 minute look, an issue or one-line PR if anything strikes them. This is preparation for the LoS conversation, not the LoS conversation itself.

### F1. Georg Krause / Funkwhale community (Germany)

**To:** governance forum or `contact@funkwhale.audio` (forum first; public engagement is more useful than DM)

**Subject:** Quick look from a fellow NGI applicant — Rust VFS with local transcription

> Hi Georg, hi Funkwhale folks,
>
> Anthony Paquet here, Canadian Rust developer. I read through Funkwhale's NLnet milestone reports last week while preparing my own NGI Zero Commons Fund application (due June 1, 2026), and the way you scope deliverables is genuinely useful — thank you for posting them publicly.
>
> I just put my project up on GitHub: `cairn`, a Rust virtual filesystem with an optional on-device transcription/tagging pipeline (FFmpeg + whisper.cpp + a small local LLM). One use case I keep coming back to is exactly Funkwhale's: a federated audio platform where individual instances might want transcripts of their podcast episodes for search, without sending anything to a third-party API.
>
> No big ask. If you have five minutes, would you take a quick look at the README and the index-pipeline trait surface in `crates/cairn-index/`? I would value either a forum reply, an issue, or a "yeah this lines up with what we would want" / "no this is the wrong shape" — whichever is honest.
>
> Repo: https://github.com/stonyp90/cairn (Codeberg mirror in README).
>
> Thanks for what you build,
> Anthony Paquet

### F2. Codeberg e.V. (Berlin, Germany)

**To:** `codeberg@codeberg.org`

**Subject:** Codeberg mirror live for a new Rust FOSS project — would value a quick look

> Hi Codeberg folks,
>
> Anthony Paquet writing from Canada. I have a new Rust workspace, `cairn`, just public on GitHub and mirrored to Codeberg. It is a privacy-first virtual filesystem with optional on-device transcription/tagging. MIT, Rust 2024.
>
> I am preparing an NLnet NGI Zero Commons Fund application (due June 1) and one of the milestone items is moving CI to Forgejo Actions on the Codeberg mirror. Before I commit to that in writing, two things would be enormously helpful:
>
> 1. A five-minute glance at the Codeberg mirror to flag anything obviously wrong (org name, repo description, README rendering on Forgejo).
> 2. If anyone on the team has time, an issue on Codeberg saying "yes this would work on our infra" or "consider X instead of Y" — that public signal of fit is what I am missing right now.
>
> Codeberg mirror: https://codeberg.org/stonyp90/cairn
> GitHub: https://github.com/stonyp90/cairn
>
> I will follow up separately about a letter of support once the README is polished. This first ask is just a sanity check on the mirror.
>
> Thanks,
> Anthony

### F3. Forgejo maintainers (Codeberg-adjacent, NGI-funded)

**To:** Forgejo Matrix room or `release@forgejo.org`

**Subject:** Rust FOSS project committing to Forgejo Actions — feedback welcome

> Hi Forgejo maintainers,
>
> Anthony, Rust dev in Canada. I just made `cairn` public — a privacy-first virtual filesystem in Rust. The roadmap (NGI Zero application underway) commits to running CI on Forgejo Actions on the Codeberg mirror, starting in M2.
>
> I have read your past NLnet milestone reports — congratulations on shipping Forgejo Actions to v1; the design notes have been useful reading. I am writing because I would rather hear "you are setting this up wrong" from you now than discover it during M2.
>
> If anyone has 10 minutes to glance at the `.gitea/workflows/` plan in the repo (currently a stub matching the GitHub Actions config), I would love an issue or PR with anything that looks off. Even "this trigger syntax is going to bite you in the way X did for project Y" is gold.
>
> Repo: https://github.com/stonyp90/cairn · Codeberg: https://codeberg.org/stonyp90/cairn
>
> Thanks, and thanks for Forgejo,
> Anthony

### F4. PeerTube / Framasoft contributor (France)

**To:** `contact@framasoft.org` (peertube subject line) or PeerTube GitHub if a specific maintainer is approachable

**Subject:** Idée d'intégration VFS / transcription locale pour PeerTube — retour bienvenu

> Bonjour,
>
> Anthony Paquet, dev Rust au Canada. Je viens de publier `cairn` sur GitHub : un système de fichiers virtuel en Rust, avec un module optionnel de transcription locale (FFmpeg + whisper.cpp) et d'étiquetage par LLM local. Aucune donnée n'est envoyée à un service tiers. Licence MIT.
>
> Je prépare une candidature NLnet NGI Zero (échéance 1er juin) et un des cas d'usage évidents est PeerTube : transcription des vidéos uploadées, sans dépendre d'une API externe. Avant d'écrire ça noir sur blanc dans la candidature, j'aimerais beaucoup avoir un retour honnête de votre part :
>
> - Est-ce que c'est une intégration que vous voudriez voir exister ?
> - Y a-t-il quelque chose dans la conception qui ne colle pas avec la façon dont PeerTube manipule ses fichiers ?
>
> Pas besoin de longue réponse. Une issue, un commentaire, ou même un "non, regarde plutôt X" suffirait. Le but est de capter un signal communautaire EU concret avant le dépôt.
>
> Dépôt : https://github.com/stonyp90/cairn (miroir Codeberg dans le README).
>
> Merci pour Framasoft,
> Anthony

---

## Distribution timeline (May 13-22, 2026)

The principle: cheap channels first, expensive channels at peak attention, and DMs steady throughout. HackerNews gets a single shot so we save it for a Tuesday/Wednesday morning North America time when the front page churns slowly enough that a thoughtful Show HN can find air.

| Day | Date | Channel | Rationale |
|-----|------|---------|-----------|
| Wed | May 13 | Mastodon launch post | Lowest cost, highest reach for the FOSS audience that already lives on the fediverse. No risk of negative front-page exposure. Seeds the link before any other channel can find it. Also a useful artifact to point Codeberg and Funkwhale at on May 14. |
| Wed | May 13 | DM #1 (Rust ecosystem dev) | Sent in evening alongside Mastodon post so the recipient sees a public signal first if they look. |
| Thu | May 14 | NLnet alumni email F2 (Codeberg) | Aligns with the EU outreach calendar in EU_COAPPLICANTS.md; gives them 24h to react to the Mastodon post if they saw it. |
| Thu | May 14 | DM #2 (EU FOSS dev) | Paired with the Codeberg email so the EU dimension has a one-day burst. |
| Fri | May 15 | NLnet alumni email F1 (Funkwhale) | Friday is fine for forum posts; weekend traffic on governance forums is actually decent. |
| Fri | May 15 | r/rust "show-and-tell" post | Friday afternoon NA time / Friday evening EU time. The Rust subreddit has weekend traffic; this gives the post a 60+ hour window to accumulate comments without competing with Monday tech news. Use the technical post, not the user-pitched one. |
| Sat | May 16 | DM #3 (privacy-focused dev) | Saturday is when long-form-reading people actually have time. Privacy-focused devs in particular skew weekend-readers. |
| Sun | May 17 | r/selfhosted post | Selfhosted subreddit has its highest traffic Sunday evening NA / Sunday late afternoon EU. Different angle from r/rust, no overlap risk. |
| Mon | May 18 | NLnet alumni email F3 (Forgejo) and F4 (Framasoft) | Monday morning EU time = peak inbox attention for EU FOSS orgs. French email to Framasoft particularly benefits from a Monday send. |
| Tue | May 19 | **HackerNews Show HN** | The single highest-attention channel. Save for Tuesday morning NA time (peak HN traffic, low competition from weekend rollover). By now there will be a small comment trail on r/rust and Mastodon that can be linked from the first comment if asked. Do not post HN on the same day as a Reddit post — splits attention and the Reddit post will get less air. |
| Wed | May 20 | First HN-comment + Mastodon follow-up post | If HN went well, share takeaways on Mastodon as a thread. If it sank, no follow-up — just keep the link clean. |
| Wed | May 20 | First follow-ups to silent NLnet alumni | Polite one-paragraph nudges to Codeberg / Funkwhale / Forgejo if no reply yet. |
| Thu | May 21 | Buffer day: respond to issues / PRs | Do not post anything new. Spend the day responding thoughtfully to any issue or PR or DM that has arrived. The point of this campaign is to convert visibility into contributors, and that conversion happens in response quality. |
| Fri | May 22 | Status check: have we hit 1-2 non-author contributors visible in `git log`? | If yes: pause outreach, focus on NLnet application. If no: send second-wave DMs to dormant Rust contacts and consider a Lobsters post (Lobsters is a smaller HN-alike that frequently turns into a contributor pipeline for Rust projects). |

**Anti-patterns to avoid:**
- Do not crosspost the same body across r/rust and r/selfhosted on the same day. Different framing per audience is the entire point.
- Do not chase metrics on Mastodon. The audience is small and serious; one engaged maintainer reading the post is worth more than 200 boosts.
- Do not respond to HN comments with "thanks!" — respond technically or do not respond. Vacuous responses lower the comment thread.
- Do not post on weekends to r/rust. The weekend mod queue + lower active reader count buries posts.

**If by May 22 the contributor count is still zero:** the problem is almost certainly the README being too dense, not the outreach being too thin. Spend May 23 trimming the README, not posting more.

# Release & Submission Playbook

The repo is fully prepared but not yet pushed. Anthony, run the steps
below in order. Each step is short and idempotent.

## Step 1 — Create the GitHub repo

```bash
gh auth status            # confirm signed in as stonyp90
gh repo create stonyp90/cairn \
  --public \
  --description "Privacy-first Rust VFS with on-device FFmpeg transcription and local-LLM auto-tagging" \
  --homepage "https://github.com/stonyp90/cairn" \
  --disable-wiki=false
```

## Step 2 — Push everything

```bash
cd /tmp/cairn
git remote add origin git@github.com:stonyp90/cairn.git
git push -u origin main
git push origin v0.1.0-alpha.1
```

## Step 3 — Set repo metadata

In the GitHub UI (Settings → General):

- Add topics: `rust`, `filesystem`, `vfs`, `fuse`, `winfsp`, `ffmpeg`,
  `whisper`, `transcription`, `privacy`, `local-first`, `nlnet`.
- Enable Issues, Discussions; disable Wiki (until M2).
- Set default branch to `main`; require PRs.

## Step 4 — Create labels and open the 15 initial issues

```bash
# Labels (run once)
gh label create "M1" --color B60205
gh label create "M2" --color D93F0B
gh label create "M3" --color FBCA04
gh label create "M4" --color 0E8A16
gh label create "M5" --color 1D76DB
gh label create "M6" --color 5319E7
gh label create "infrastructure" --color c5def5
gh label create "docs" --color 0075ca
# good first issue / help wanted exist by default
```

Then open each issue from `docs/INITIAL_ISSUES.md` via `gh issue create`.
A small script is provided at `scripts/open-initial-issues.sh` (TODO —
write it during the actual submission week).

## Step 5 — Create the Codeberg EU mirror

```bash
# After registering at codeberg.org
git remote add codeberg git@codeberg.org:stonyp90/cairn.git
git push -u codeberg main
git push codeberg v0.1.0-alpha.1
```

Set the Codeberg repo to "Mirror" so it auto-syncs from GitHub on every
push. Add a one-line note in the README pointing at the EU mirror.

## Step 6 — EU letter-of-support outreach (T-21 to T-14 days)

See `docs/EU_COAPPLICANTS.md` for the ranked candidates and the
personalised outreach template. Email **Codeberg e.V. first**, then
**FSFE**, then the lower-likelihood candidates in parallel. Goal: at
least one signed LoS by T-3 days.

## Step 7 — Submit the NLnet application

- Open https://nlnet.nl/propose/ .
- Paste from `docs/NLNET_APPLICATION_DRAFT.md` into the matching form
  fields. The draft headings track the NLnet fields 1:1.
- Attach the signed LoS PDF in the "supporting documents" field.
- Submit **before 31 May 2026 23:00 EDT** (= 1 June 03:00 UTC, ≈ 9 hours
  before the official deadline). Buffer guards against form bugs.
- Save the submission confirmation email to
  `_offline/nlnet_submission_confirmation.eml` for the record.

## Step 8 — After submission

- Reply to the auto-acknowledgement within 24h to confirm receipt.
- Continue committing to the repo at a normal cadence (1–3 commits per
  week). Reviewers occasionally re-check repo activity during the
  evaluation window.
- Expected first response from NLnet: ~6–10 weeks.

## Failure modes and what to do

- **NLnet form rejects long fields.** Re-paste the section trimmed of
  the bracketed citations and links.
- **Codeberg LoS not received by T-7.** Switch to FSFE as the
  primary EU anchor and reference the Codeberg mirror commitment as
  evidence of EU dimension regardless.
- **GitHub repo flagged for review.** The MIT licence + no
  trademarked names is deliberately conservative; this should not
  happen. If it does, escalate to GitHub support and proceed with
  the Codeberg primary URL on the NLnet form.

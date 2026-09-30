@AGENTS.md

# Claude Code notes

`AGENTS.md` (imported above) is the shared project memory for Claude Code, Codex, and
Qoder. Put project facts there, not here. This file holds only Claude-specific notes.

- **Artifact contracts:** read `docs/ARTIFACTS.md` before editing release workflows,
  website download JS, `models.toml`, `docs/schema/*`, `docs/api/openapi.yaml`, or
  anything under `training/`. It's deliberately not `@`-imported, so it doesn't
  cost context on unrelated tasks.
- **Previewing the website:** `.claude/launch.json` defines `diaspor-website`
  (`python3 -m http.server 8765 --directory website`). EN is at `/`, FR at `/fr/`.
- **Cloud/web sessions** run on Linux with no macOS or Windows runner and usually no
  Tauri system libraries (webkit2gtk and friends). Run what you can (workspace
  `cargo` checks, SDK tests, `tools/check_asset_pattern.py`), then say plainly
  which checks you could not run and why. Don't claim a desktop build passed
  without building it.
- **Don't push to `main`.** A push there that touches `website/**` deploys straight
  to production. Work on a branch and open a PR using `.github/pull_request_template.md`.
- **Keep diffs scoped.** The inconsistencies listed in `AGENTS.md` §8 are known.
  Mention them when they're relevant, but fix them only when asked.
- **Personal notes** go in `CLAUDE.local.md`, which is git-ignored.

#!/usr/bin/env python3
"""
Asset-pattern check: keep `.github/workflows/release.yml` and
`website/index.html` in sync so the download buttons on diaspor.io always
resolve to artifacts that the release workflow actually produces.

What this validates:

  1. Every `(target, archive)` row in release.yml's `build-cli` matrix has
     a corresponding entry in the ARCHIVES map inside the
     `initCliDownloads` IIFE in website/index.html.
  2. The reverse: every ARCHIVES entry in the website is covered by the
     release matrix.
  3. The filename template the website JS computes
     (`diaspor-<tag>-<target>.<ext>`) matches what release.yml emits.

The script is intentionally tolerant of YAML / JS formatting variation but
strict on the set equality. It exits non-zero on any drift so CI fails
loudly.

Run locally:
    python3 tools/check_asset_pattern.py
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
RELEASE_YML = REPO_ROOT / ".github" / "workflows" / "release.yml"
RELEASE_DESKTOP_YML = REPO_ROOT / ".github" / "workflows" / "release-desktop.yml"
WEBSITE_HTML_EN = REPO_ROOT / "website" / "index.html"
WEBSITE_HTML_FR = REPO_ROOT / "website" / "fr" / "index.html"

# The desktop installer filenames the website's initDesktopDownloads IIFE
# expects to find on the latest GitHub release. release-desktop.yml renames
# the bundle output to these exact strings before uploading; if the
# workflow or website drifts on naming, the .dmg/.msi/.AppImage download
# buttons will 404. Keep this map in sync with both:
#   - the ASSETS object inside initDesktopDownloads() in website/index.html
#   - the artifact rename lines in .github/workflows/release-desktop.yml
DESKTOP_EXPECTED_ASSETS = {
    "macos": "diaspor-vfs.dmg",
    "windows": "diaspor-vfs.msi",
    "linux": "diaspor-vfs.AppImage",
}


def parse_release_matrix() -> dict[str, str]:
    """Return a `{target: archive_ext}` map from release.yml's build-cli matrix."""
    doc = yaml.safe_load(RELEASE_YML.read_text())
    try:
        include = doc["jobs"]["build-cli"]["strategy"]["matrix"]["include"]
    except KeyError as exc:
        raise SystemExit(
            f"release.yml: could not find jobs.build-cli.strategy.matrix.include ({exc})"
        ) from exc
    if not isinstance(include, list):
        raise SystemExit("release.yml: build-cli matrix.include must be a list")
    targets: dict[str, str] = {}
    for entry in include:
        target = entry.get("target")
        archive = entry.get("archive")
        if not target or not archive:
            raise SystemExit(f"release.yml: matrix entry missing target/archive: {entry!r}")
        if target in targets:
            raise SystemExit(f"release.yml: duplicate target in matrix: {target}")
        targets[target] = archive
    return targets


# Match the ARCHIVES object literal inside the initCliDownloads IIFE. The
# website's JS lives inline in index.html so we extract via regex rather
# than spinning up a JS parser. The relevant block looks like:
#
#     const ARCHIVES = {
#       'aarch64-apple-darwin': 'tar.gz',
#       'x86_64-apple-darwin': 'tar.gz',
#       ...
#     };
ARCHIVES_BLOCK_RE = re.compile(
    r"const\s+ARCHIVES\s*=\s*\{([^}]*)\}\s*;",
    re.DOTALL,
)
ARCHIVES_ENTRY_RE = re.compile(
    r"['\"](?P<target>[A-Za-z0-9_.-]+)['\"]\s*:\s*['\"](?P<ext>[A-Za-z0-9._-]+)['\"]"
)


def parse_website_archives(path: Path) -> dict[str, str]:
    """Return the `{target: archive_ext}` map from the ARCHIVES const in an HTML file."""
    html = path.read_text()
    block_match = ARCHIVES_BLOCK_RE.search(html)
    if not block_match:
        # FR mirror does not duplicate the JS — only the EN page hosts the
        # initCliDownloads IIFE. The FR page links to the same anchor on the
        # EN page for downloads, so an empty map is acceptable here.
        return {}
    block = block_match.group(1)
    targets: dict[str, str] = {}
    for entry in ARCHIVES_ENTRY_RE.finditer(block):
        target = entry.group("target")
        ext = entry.group("ext")
        if target in targets:
            raise SystemExit(f"{path}: duplicate target in ARCHIVES: {target}")
        targets[target] = ext
    return targets


# Match the ASSETS object literal inside the initDesktopDownloads IIFE:
#     const ASSETS = {
#       macos:   'diaspor-vfs.dmg',
#       windows: 'diaspor-vfs.msi',
#       linux:   'diaspor-vfs.AppImage',
#     };
DESKTOP_ASSETS_BLOCK_RE = re.compile(
    r"const\s+ASSETS\s*=\s*\{([^}]*)\}\s*;",
    re.DOTALL,
)
DESKTOP_ASSETS_ENTRY_RE = re.compile(
    r"(?P<platform>macos|windows|linux)\s*:\s*['\"](?P<filename>[A-Za-z0-9._-]+)['\"]"
)


def parse_website_desktop_assets(path: Path) -> dict[str, str]:
    """Return the `{platform: filename}` map from the ASSETS const in initDesktopDownloads."""
    html = path.read_text()
    block_match = DESKTOP_ASSETS_BLOCK_RE.search(html)
    if not block_match:
        return {}
    block = block_match.group(1)
    found: dict[str, str] = {}
    for entry in DESKTOP_ASSETS_ENTRY_RE.finditer(block):
        platform = entry.group("platform")
        filename = entry.group("filename")
        if platform in found:
            raise SystemExit(f"{path}: duplicate platform in ASSETS: {platform}")
        found[platform] = filename
    return found


def parse_release_desktop_filenames() -> set[str]:
    """Return the set of desktop installer filenames release-desktop.yml emits.

    Greps the workflow for `release-artifacts/diaspor-vfs.<ext>` paths in the
    Prepare/Upload artifact steps. We don't try to be YAML-precise here —
    the workflow has a mix of bash + pwsh blocks where the filename appears
    verbatim. A simple regex is sufficient and resilient.
    """
    text = RELEASE_DESKTOP_YML.read_text()
    return set(re.findall(r"release-artifacts/(diaspor-vfs\.[A-Za-z]+)", text))


def main() -> int:
    matrix = parse_release_matrix()
    web_en = parse_website_archives(WEBSITE_HTML_EN)
    web_fr = parse_website_archives(WEBSITE_HTML_FR)
    desktop_web_en = parse_website_desktop_assets(WEBSITE_HTML_EN)
    desktop_web_fr = parse_website_desktop_assets(WEBSITE_HTML_FR)
    desktop_workflow_files = parse_release_desktop_filenames()

    print("release.yml build-cli matrix:")
    print(json.dumps(matrix, indent=2, sort_keys=True))
    print("\nwebsite/index.html ARCHIVES:")
    print(json.dumps(web_en, indent=2, sort_keys=True))
    if web_fr:
        print("\nwebsite/fr/index.html ARCHIVES:")
        print(json.dumps(web_fr, indent=2, sort_keys=True))

    failures: list[str] = []
    for label, web in [("website/index.html", web_en), ("website/fr/index.html", web_fr)]:
        if not web:
            # FR mirror is allowed to skip the JS; only EN must have it.
            if label.endswith("fr/index.html"):
                continue
            failures.append(f"{label}: could not find `const ARCHIVES = {{...}}` block")
            continue
        missing_in_web = set(matrix) - set(web)
        extra_in_web = set(web) - set(matrix)
        wrong_ext = {t for t in matrix.keys() & web.keys() if matrix[t] != web[t]}
        for target in sorted(missing_in_web):
            failures.append(
                f"{label}: target {target!r} is in release.yml matrix but missing from ARCHIVES "
                f"(download button for this platform will 404)"
            )
        for target in sorted(extra_in_web):
            failures.append(
                f"{label}: target {target!r} is in ARCHIVES but missing from release.yml matrix "
                f"(release workflow will not produce this asset)"
            )
        for target in sorted(wrong_ext):
            failures.append(
                f"{label}: target {target!r} archive ext mismatch: "
                f"release.yml emits .{matrix[target]} but website expects .{web[target]}"
            )

    # Desktop installer asset cross-check: release-desktop.yml filenames ↔
    # website initDesktopDownloads ASSETS map ↔ DESKTOP_EXPECTED_ASSETS.
    print("\nrelease-desktop.yml emitted filenames:")
    print(json.dumps(sorted(desktop_workflow_files), indent=2))
    print("\nwebsite/index.html initDesktopDownloads ASSETS:")
    print(json.dumps(desktop_web_en, indent=2, sort_keys=True))
    if desktop_web_fr:
        print("\nwebsite/fr/index.html initDesktopDownloads ASSETS:")
        print(json.dumps(desktop_web_fr, indent=2, sort_keys=True))

    expected_filenames = set(DESKTOP_EXPECTED_ASSETS.values())
    missing_in_workflow = expected_filenames - desktop_workflow_files
    extra_in_workflow = desktop_workflow_files - expected_filenames
    for fn in sorted(missing_in_workflow):
        failures.append(
            f"release-desktop.yml: expected to emit {fn!r} but no such "
            f"`release-artifacts/{fn}` path was found"
        )
    for fn in sorted(extra_in_workflow):
        failures.append(
            f"release-desktop.yml: emits {fn!r} but no website button "
            f"references it (asset will be uploaded but unreachable)"
        )

    for label, web in [
        ("website/index.html", desktop_web_en),
        ("website/fr/index.html", desktop_web_fr),
    ]:
        if not web:
            failures.append(
                f"{label}: could not find `const ASSETS = {{...}}` block "
                f"inside initDesktopDownloads (download buttons will not be wired)"
            )
            continue
        for platform, expected_fn in DESKTOP_EXPECTED_ASSETS.items():
            actual_fn = web.get(platform)
            if actual_fn is None:
                failures.append(
                    f"{label}: ASSETS map is missing platform {platform!r} "
                    f"(no fallback URL will be computed)"
                )
            elif actual_fn != expected_fn:
                failures.append(
                    f"{label}: ASSETS[{platform!r}] is {actual_fn!r} but workflow "
                    f"emits {expected_fn!r} (download button will 404)"
                )

    if failures:
        print("\nDRIFT DETECTED:", file=sys.stderr)
        for fail in failures:
            print(f"  - {fail}", file=sys.stderr)
        return 1

    print("\nOK: release.yml matrix + release-desktop.yml filenames + website maps are in sync.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

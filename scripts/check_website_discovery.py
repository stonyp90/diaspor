#!/usr/bin/env python3
"""Fail before deployment when Diaspor's public discovery contract regresses."""

import argparse
import json
import re
import sys
import xml.etree.ElementTree as ET
from html.parser import HTMLParser
from pathlib import Path

ORIGIN = "https://diaspor.io"
PAGES = {"index.html": ("en", ORIGIN + "/"), "fr/index.html": ("fr", ORIGIN + "/fr/")}
ALTERNATES = {"en": ORIGIN + "/", "fr": ORIGIN + "/fr/", "x-default": ORIGIN + "/"}
PROOF = "google5781a30bbd14f93a.html"
PRIVATE_PATHS = {"/api", "/admin", "/app", "/account"}


class Page(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.in_head = False
        self.capture = None
        self.buffer = []
        self.lang = None
        self.meta = {}
        self.canonicals = []
        self.alternates = []
        self.titles = []
        self.headings = []
        self.schemas = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "html":
            self.lang = attrs.get("lang")
        if tag == "head":
            self.in_head = True
        if tag == "h1" or (self.in_head and tag == "title"):
            self.capture, self.buffer = tag, []
        if self.in_head and tag == "script" and attrs.get("type") == "application/ld+json":
            self.capture, self.buffer = "schema", []
        if self.in_head and tag == "meta":
            key = attrs.get("name", attrs.get("property", "")).lower()
            self.meta.setdefault(key, []).append(attrs.get("content", ""))
        if self.in_head and tag == "link":
            rel = attrs.get("rel", "").split()
            if "canonical" in rel:
                self.canonicals.append(attrs.get("href"))
            if "alternate" in rel and "hreflang" in attrs:
                self.alternates.append((attrs["hreflang"], attrs.get("href")))

    def handle_data(self, data):
        if self.capture:
            self.buffer.append(data)

    def handle_endtag(self, tag):
        if (tag == self.capture) or (tag == "script" and self.capture == "schema"):
            text = "".join(self.buffer).strip()
            if self.capture == "title":
                self.titles.append(text)
            elif self.capture == "h1":
                self.headings.append(" ".join(text.split()))
            else:
                self.schemas.append(text)
            self.capture, self.buffer = None, []
        if tag == "head":
            self.in_head = False


def check(website):
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    for filename, (language, canonical) in PAGES.items():
        path = website / filename
        if not path.is_file():
            errors.append(f"{filename}: missing public page")
            continue
        page = Page()
        page.feed(path.read_text(encoding="utf-8"))
        require(page.lang == language, f"{filename}: incorrect document language")
        require(len(page.titles) == 1 and bool(page.titles[0].strip()), f"{filename}: needs one nonempty title")
        require(len(page.headings) == 1 and bool(page.headings[0]), f"{filename}: needs one nonempty H1")
        descriptions = page.meta.get("description", [])
        require(len(descriptions) == 1 and bool(descriptions[0].strip()), f"{filename}: needs one description")
        require(page.canonicals == [canonical], f"{filename}: canonical must be {canonical}")
        require(len(page.alternates) == len(ALTERNATES) and dict(page.alternates) == ALTERNATES,
                f"{filename}: EN, FR and x-default alternates must be reciprocal")
        robots = page.meta.get("robots", [])
        directives = {part.strip().lower() for value in robots for part in value.split(",")}
        require(len(robots) == 1 and {"index", "follow"}.issubset(directives)
                and not {"noindex", "nofollow", "none"}.intersection(directives),
                f"{filename}: public page must permit index, follow")
        for key in ("googlebot", "bingbot"):
            bot_rules = {part.strip().lower() for value in page.meta.get(key, []) for part in value.split(",")}
            require(not {"noindex", "nofollow", "none"}.intersection(bot_rules), f"{filename}: {key} blocks discovery")
        for key in ("og:url", "twitter:url"):
            require(page.meta.get(key) == [canonical], f"{filename}: {key} must match canonical")
        for key in ("og:title", "twitter:title"):
            require(page.meta.get(key) == page.titles, f"{filename}: {key} must match title")
        for key in ("og:description", "twitter:description"):
            require(page.meta.get(key) == descriptions, f"{filename}: {key} must match description")
        require(page.meta.get("og:locale") == [language + "_CA"], f"{filename}: incorrect social locale")
        require(bool(page.schemas), f"{filename}: missing JSON-LD")
        for text in page.schemas:
            try:
                schema = json.loads(text)
                require(isinstance(schema, dict), f"{filename}: JSON-LD must be an object")
                if isinstance(schema, dict):
                    require(schema.get("@context") == "https://schema.org", f"{filename}: invalid schema context")
                    require(schema.get("@type") == "SoftwareApplication" and schema.get("name") == "Diaspor",
                            f"{filename}: incorrect product identity")
                    require(schema.get("url") == canonical, f"{filename}: JSON-LD URL must match canonical")
                    require(schema.get("inLanguage") == language + "-CA", f"{filename}: incorrect schema language")
                    require(not {"aggregateRating", "review", "offers"}.intersection(schema),
                            f"{filename}: do not add unverified ratings, reviews or prices")
            except json.JSONDecodeError as exc:
                errors.append(f"{filename}: invalid JSON-LD: {exc}")

    try:
        lines = (website / "robots.txt").read_text(encoding="utf-8").splitlines()
        rules = []
        for line in lines:
            line = line.split("#", 1)[0]
            if ":" in line:
                key, value = line.split(":", 1)
                rules.append((key.strip().lower(), value.strip()))
        require([value for key, value in rules if key == "user-agent"] == ["*"], "robots.txt: use one shared wildcard group")
        require(("allow", "/") in rules, "robots.txt: public pages must be allowed")
        disallows = {value for key, value in rules if key == "disallow"}
        require(disallows == PRIVATE_PATHS, "robots.txt: expected shared api/admin/app/account exclusions")
        require(("sitemap", ORIGIN + "/sitemap.xml") in rules, "robots.txt: missing canonical sitemap")
        require(not any(key == "crawl-delay" for key, _ in rules), "robots.txt: remove crawl-delay")
    except OSError as exc:
        errors.append(f"robots.txt: {exc}")

    try:
        ns = {"s": "http://www.sitemaps.org/schemas/sitemap/0.9", "x": "http://www.w3.org/1999/xhtml"}
        tree = ET.parse(website / "sitemap.xml")
        entries = tree.findall("s:url", ns)
        urls = [entry.findtext("s:loc", namespaces=ns) for entry in entries]
        require(len(urls) == len(PAGES) and set(urls) == {url for _, url in PAGES.values()},
                "sitemap.xml: exactly the two public canonical URLs are required")
        for entry in entries:
            require(not any(entry.find("s:" + tag, ns) is not None for tag in ("lastmod", "priority", "changefreq")),
                    "sitemap.xml: omit invented dates and scheduling hints")
            alternates = [(link.get("hreflang"), link.get("href")) for link in entry.findall("x:link", ns)
                          if link.get("rel") == "alternate"]
            require(len(alternates) == len(ALTERNATES) and dict(alternates) == ALTERNATES,
                    "sitemap.xml: every entry needs reciprocal language alternates")
    except (OSError, ET.ParseError) as exc:
        errors.append(f"sitemap.xml: {exc}")

    try:
        guide = (website / "llms.txt").read_text(encoding="utf-8")
        urls = set(re.findall(r"https?://[^\s)]+", guide))
        require(urls == {url for _, url in PAGES.values()}, "llms.txt: link only to the two actual public pages")
        require("early access" in guide.lower() and "accès anticipé" in guide.lower(), "llms.txt: preserve early-access status in both languages")
    except OSError as exc:
        errors.append(f"llms.txt: {exc}")
    try:
        require((website / PROOF).read_bytes() == ("google-site-verification: " + PROOF).encode("ascii"),
                f"{PROOF}: verification file must match the exact 53 bytes, without a newline")
    except OSError as exc:
        errors.append(f"{PROOF}: {exc}")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--website-dir", type=Path, default=Path(__file__).resolve().parents[1] / "website")
    args = parser.parse_args()
    errors = check(args.website_dir)
    if errors:
        for error in errors:
            print("FAIL: " + error, file=sys.stderr)
        return 1
    print("PASS: 2 public pages; canonical metadata, reciprocal languages, JSON-LD, robots, sitemap, llms and exact Google verification bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

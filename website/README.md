# diaspor.io marketing website

Static landing site for **diaspor.io**. No build step — plain HTML + CSS, deployed to S3 + CloudFront in `ca-central-1`.

## Layout

```
website/
├── index.html              English landing page
├── fr/index.html           French landing page
├── styles.css              Single stylesheet (~244KB)
├── favicon.svg, logo.svg   Brand assets
├── site.webmanifest        PWA manifest
├── sitemap.xml, robots.txt SEO
├── deploy.sh                       S3 sync + CloudFront invalidation
└── deploy-with-credentials.sh      Same, but reads creds from a file
```

## Local dev

```bash
python3 -m http.server 8765 --directory /Users/tony/stonyp90-websites/diaspor/website
# → http://localhost:8765/   (EN)
# → http://localhost:8765/fr/ (FR)
```

The Claude Code preview launcher is wired up at `.claude/launch.json` (name: `diaspor-site`).

## Deploy

```bash
AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=... ./deploy.sh
```

Defaults target the live diaspor.io stack:
- S3 bucket: `diaspor-io-site-436136277668`
- CloudFront distribution: `E5ZB29XQZG1PT`
- Region: `ca-central-1`

Override via env vars (`S3_BUCKET`, `CLOUDFRONT_DISTRIBUTION_ID`, `AWS_REGION`).

## Responsive contract

The site is verified against these viewport widths: **320, 360, 375, 390, 414, 480, 568, 640, 768, 820, 1024, 1180, 1280, 1440**. The contract:
- Zero horizontal scroll on `<body>` at any width
- All interactive elements ≥ 44×44 CSS pixels
- WCAG 2.1 AA contrast on every visible text color (≥ 4.5:1 body, ≥ 3:1 large)
- No content gated behind `:hover` (touch parity via `:focus-visible` + `<details>`)
- Mobile menu opens at < 769px, locks body scroll, traps focus, closes on Escape

Mobile-specific polish lives at the bottom of `styles.css` under the banner
`Mobile + a11y polish (appended 2026-05-16)` — keep new responsive overrides
in that block so they're easy to find and review.

## Provenance

Copied 2026-05-16 from `stonyp90-websites/ursly/website/` (the deployed
`ursly.io` site, already fully Diaspor-branded). The `ursly/` tree is frozen
until the Q3 2026 `app.ursly.io → app.diaspor.io` cutover — do not commit
matching changes back into it.

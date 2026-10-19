# Public website discovery: standing launch policy

Apply this policy by default to future Diaspor product and public website changes.

The current public catalog is `https://diaspor.io/` (English) and
`https://diaspor.io/fr/` (French). Both are public early-access marketing pages.
Keep the release status, limitations and contact workflow accurate in the visible
copy, page metadata, JSON-LD and `llms.txt`. Do not invent prices, ratings,
certifications, customer counts, availability or crawler endorsements.

Every new public page needs useful HTML content, one title, description and H1,
a self-canonical URL, consistent social metadata and truthful structured data.
Where translations exist, make language alternates reciprocal, including each
page itself. Update the public route catalog in
`scripts/check_website_discovery.py`, the sitemap and the discovery guide together.
Use a real content-change date if adding `lastmod`; do not stamp deployment time.

Public pages permit indexing and following. Keep API, admin, app and account
paths outside the public catalog. Their robots exclusions are crawl guidance,
not authentication. If private routes are introduced, enforce access control
and add appropriate noindex response metadata. Keep one shared crawler policy;
do not accidentally override exclusions with separate agent groups. Changing a
model-training policy requires a separate explicit decision.

`llms.txt` is an optional factual navigation guide. It does not guarantee that an
assistant will read, train on, index or cite the site. Search inclusion and
ranking remain decisions made by external services.

## Required local and release checks

From the repository root:

```sh
python3 scripts/check_website_discovery.py
bash -n website/deploy.sh
npx --yes --package=html-validate@8 html-validate website/index.html website/fr/index.html
```

The Python guard uses only the standard library. It validates both pages,
canonical and social metadata, reciprocal languages, JSON-LD, the shared robots
policy, the exact sitemap catalog, guide links and the exact Google verification
bytes. Both deployment paths run it before uploads. The CI deployment also runs
HTML validation. No application build is required for these static files.

Both the manual script and GitHub deployment must upload `robots.txt`,
`sitemap.xml`, `llms.txt`, and `google5781a30bbd14f93a.html`. Discovery text and XML
use a one-hour cache with revalidation; the verification HTML uses a five-minute
cache. Keep verification bytes unchanged, with no terminal newline. Do not put
discovery files in the immutable asset loop.

After an authorized production deployment, verify HTTP 200 and the expected
content for both canonical pages and all four discovery/verification files.
Check for conflicting response-level `X-Robots-Tag`, redirects, authentication,
CDN rules or blocked crawlers. Confirm the English and French rendered metadata,
complete Google ownership verification and submit the canonical sitemap in the
available search consoles. Record the actual deployed commit and submission
results; source checks alone do not prove live indexing or availability.

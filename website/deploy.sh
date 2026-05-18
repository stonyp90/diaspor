#!/bin/bash
set -euo pipefail

# Website deployment script for diaspor.io
# Deploys static website files to AWS S3 + invalidates CloudFront.

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration — defaults target the live diaspor.io stack.
S3_BUCKET="${S3_BUCKET:-diaspor-io-site-436136277668}"
CLOUDFRONT_DISTRIBUTION_ID="${CLOUDFRONT_DISTRIBUTION_ID:-E5ZB29XQZG1PT}"
REGION="${AWS_REGION:-ca-central-1}"

echo -e "${GREEN}🚀 Deploying website to S3...${NC}"
echo "Bucket: $S3_BUCKET"
echo "Region: $REGION"

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    echo -e "${RED}❌ AWS CLI is not installed. Please install it first.${NC}"
    exit 1
fi

# Check if credentials are set
if [ -z "${AWS_ACCESS_KEY_ID:-}" ] || [ -z "${AWS_SECRET_ACCESS_KEY:-}" ]; then
    echo -e "${RED}❌ AWS credentials not set. Please set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY.${NC}"
    exit 1
fi

# Change to website directory
cd "$(dirname "$0")"

# Verify we're in the right directory
if [ ! -f "index.html" ]; then
    echo -e "${RED}❌ index.html not found. Are you in the website directory?${NC}"
    exit 1
fi

echo -e "${GREEN}📦 Syncing files to S3...${NC}"

# Sync files to S3 with proper content types and cache headers
aws s3 sync . s3://$S3_BUCKET \
    --region $REGION \
    --delete \
    --exclude "*.sh" \
    --exclude ".git/*" \
    --exclude ".gitignore" \
    --exclude "deploy.sh" \
    --cache-control "public, max-age=31536000, immutable" \
    --exclude "*.html" \
    --exclude "*.css" \
    --exclude "*.js" \
    --exclude "*.json" \
    --exclude "*.xml" \
    --exclude "*.txt" \
    --exclude "*.webmanifest"

# Cache-bust styles.css in the HTML by appending a content hash.
# styles.css is uploaded with max-age=31536000, immutable, so once a browser
# caches it the only way to force a refresh is a new URL.
CSS_HASH=$(md5 -q styles.css 2>/dev/null || md5sum styles.css | awk '{print $1}')
CSS_HASH=${CSS_HASH:0:8}

# Build marker — verifiable proof of which commit is on prod.
# In CI, $GITHUB_SHA is set; locally we fall back to `git rev-parse HEAD`.
BUILD_SHA="${GITHUB_SHA:-$(git -C "$(dirname "$0")/.." rev-parse HEAD 2>/dev/null || echo unknown)}"
BUILD_SHA_SHORT="${BUILD_SHA:0:8}"
BUILD_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
BUILD_MARKER="<!-- build: ${BUILD_SHA_SHORT} at ${BUILD_TIMESTAMP} -->"

HTML_TMP="$(mktemp)"
trap 'rm -f "$HTML_TMP"' EXIT
sed \
    -e "s|href=\"styles.css\"|href=\"styles.css?v=${CSS_HASH}\"|" \
    -e "s|</head>|${BUILD_MARKER}</head>|" \
    -e "s|<html lang=\"en\">|<html lang=\"en\" data-build=\"${BUILD_SHA_SHORT}\">|" \
    index.html > "$HTML_TMP"
echo -e "${GREEN}🔖 styles.css hash: ${CSS_HASH}${NC}"
echo -e "${GREEN}🏷️  build marker: ${BUILD_SHA_SHORT} @ ${BUILD_TIMESTAMP}${NC}"

# Upload HTML files with shorter cache (they change more frequently)
echo -e "${GREEN}📄 Uploading HTML files...${NC}"
aws s3 cp "$HTML_TMP" s3://$S3_BUCKET/index.html \
    --region $REGION \
    --content-type "text/html; charset=utf-8" \
    --cache-control "public, max-age=0, must-revalidate"

# French page — same cache-bust + build marker treatment as the root.
if [ -f "fr/index.html" ]; then
    FR_HTML_TMP="$(mktemp)"
    trap 'rm -f "$HTML_TMP" "$FR_HTML_TMP"' EXIT
    sed \
        -e "s|href=\"/styles.css\"|href=\"/styles.css?v=${CSS_HASH}\"|" \
        -e "s|href=\"styles.css\"|href=\"styles.css?v=${CSS_HASH}\"|" \
        -e "s|</head>|${BUILD_MARKER}</head>|" \
        -e "s|<html lang=\"fr\">|<html lang=\"fr\" data-build=\"${BUILD_SHA_SHORT}\">|" \
        fr/index.html > "$FR_HTML_TMP"
    aws s3 cp "$FR_HTML_TMP" s3://$S3_BUCKET/fr/index.html \
        --region $REGION \
        --content-type "text/html; charset=utf-8" \
        --cache-control "public, max-age=0, must-revalidate"
fi

# Upload CSS files
echo -e "${GREEN}🎨 Uploading CSS files...${NC}"
aws s3 cp styles.css s3://$S3_BUCKET/styles.css \
    --region $REGION \
    --content-type "text/css; charset=utf-8" \
    --cache-control "public, max-age=31536000, immutable"

# Upload JSON files
if [ -f "site.webmanifest" ]; then
    echo -e "${GREEN}📋 Uploading web manifest...${NC}"
    aws s3 cp site.webmanifest s3://$S3_BUCKET/site.webmanifest \
        --region $REGION \
        --content-type "application/manifest+json" \
        --cache-control "public, max-age=3600"
fi

# Upload SVG files with proper content type
echo -e "${GREEN}🖼️ Uploading SVG files...${NC}"
for svg in *.svg; do
    if [ -f "$svg" ]; then
        aws s3 cp "$svg" s3://$S3_BUCKET/"$svg" \
            --region $REGION \
            --content-type "image/svg+xml" \
            --cache-control "public, max-age=31536000, immutable"
    fi
done

# Upload robots.txt and sitemap.xml (excluded from the initial sync above)
if [ -f "robots.txt" ]; then
    echo -e "${GREEN}🤖 Uploading robots.txt...${NC}"
    aws s3 cp robots.txt s3://$S3_BUCKET/robots.txt \
        --region $REGION \
        --content-type "text/plain; charset=utf-8" \
        --cache-control "public, max-age=3600"
fi

if [ -f "sitemap.xml" ]; then
    echo -e "${GREEN}🗺️  Uploading sitemap.xml...${NC}"
    aws s3 cp sitemap.xml s3://$S3_BUCKET/sitemap.xml \
        --region $REGION \
        --content-type "application/xml; charset=utf-8" \
        --cache-control "public, max-age=3600"
fi

# Upload screenshots directory
if [ -d "screenshots" ]; then
    echo -e "${GREEN}📸 Uploading screenshots...${NC}"
    aws s3 sync screenshots/ s3://$S3_BUCKET/screenshots/ \
        --region $REGION \
        --cache-control "public, max-age=31536000, immutable"
else
    echo -e "${YELLOW}🗑  No local screenshots/ — removing stale objects from S3...${NC}"
    aws s3 rm "s3://$S3_BUCKET/screenshots/" --recursive --region "$REGION" 2>/dev/null || true
fi

echo -e "${GREEN}✅ Files uploaded successfully!${NC}"

# Invalidate CloudFront cache if distribution ID is provided
if [ -n "$CLOUDFRONT_DISTRIBUTION_ID" ]; then
    echo -e "${YELLOW}🔄 Invalidating CloudFront cache...${NC}"
    INVALIDATION_ID=$(aws cloudfront create-invalidation \
        --distribution-id "$CLOUDFRONT_DISTRIBUTION_ID" \
        --paths "/*" \
        --region $REGION \
        --query 'Invalidation.Id' \
        --output text)
    
    echo -e "${GREEN}✅ CloudFront invalidation created: $INVALIDATION_ID${NC}"
    echo -e "${YELLOW}⏳ Cache invalidation may take a few minutes to complete.${NC}"
else
    echo -e "${YELLOW}⚠️  CLOUDFRONT_DISTRIBUTION_ID not set. Skipping cache invalidation.${NC}"
    echo -e "${YELLOW}   Set CLOUDFRONT_DISTRIBUTION_ID environment variable to invalidate CloudFront cache.${NC}"
fi

echo -e "${GREEN}🎉 Deployment complete!${NC}"
echo -e "${GREEN}   Website: https://$S3_BUCKET${NC}"

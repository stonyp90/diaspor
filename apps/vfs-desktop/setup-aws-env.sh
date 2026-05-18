#!/bin/bash

# AWS S3 Configuration template for local development.
#
# Copy this file to setup-aws-env.local.sh (which is gitignored) and fill
# in real credentials there. Then `source ./setup-aws-env.local.sh` before
# running the app. Never commit real keys to git — GitHub push protection
# will block the push and the leaked credentials must then be rotated.

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-AKIA_REPLACE_ME}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-replace-me}"
export AWS_REGION="${AWS_REGION:-us-east-2}"
export AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-us-east-2}"

# S3 Test Bucket Configuration
export DIASPOR_S3_TEST_BUCKET="${DIASPOR_S3_TEST_BUCKET:-diaspor-vfs-test}"
export DIASPOR_S3_TEST_REGION="${DIASPOR_S3_TEST_REGION:-us-east-2}"

echo "✓ AWS environment variables set (from template — override locally):"
echo "  - AWS_ACCESS_KEY_ID: ${AWS_ACCESS_KEY_ID:0:10}..."
echo "  - AWS_REGION: $AWS_REGION"
echo "  - S3 Test Bucket: $DIASPOR_S3_TEST_BUCKET"
echo ""
echo "Ready to run: npm run tauri:dev"

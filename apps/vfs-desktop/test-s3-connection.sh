#!/bin/bash

# Test AWS S3 Connection
# This script verifies AWS credentials and S3 bucket access

echo "🔍 Testing AWS S3 Connection..."
echo ""

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    echo "⚠️  AWS CLI not found. Installing is optional but recommended for testing."
    echo "   Install: https://aws.amazon.com/cli/"
    echo ""
fi

# Source the environment variables
if [ -f "./setup-aws-env.sh" ]; then
    source ./setup-aws-env.sh
    echo ""
else
    echo "❌ setup-aws-env.sh not found!"
    echo "   Please create it first."
    exit 1
fi

# Check if environment variables are set
if [ -z "$AWS_ACCESS_KEY_ID" ] || [ -z "$AWS_SECRET_ACCESS_KEY" ]; then
    echo "❌ AWS credentials not set!"
    echo "   Make sure setup-aws-env.sh exports AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"
    exit 1
fi

echo "✓ Environment variables loaded"
echo ""

# Test AWS CLI connection if available
if command -v aws &> /dev/null; then
    echo "📡 Testing AWS connection..."
    
    # Test S3 bucket access
    if aws s3 ls "s3://$URSLY_S3_TEST_BUCKET" --region "$AWS_REGION" 2>&1 | grep -q "An error occurred"; then
        echo "❌ Cannot access S3 bucket: $URSLY_S3_TEST_BUCKET"
        echo "   Check your credentials and bucket name"
        exit 1
    else
        echo "✅ Successfully connected to S3 bucket: $URSLY_S3_TEST_BUCKET"
        echo ""
        echo "📦 Bucket contents:"
        aws s3 ls "s3://$URSLY_S3_TEST_BUCKET/" --region "$AWS_REGION" | head -10
        echo ""
    fi
else
    echo "⚠️  AWS CLI not installed, skipping connection test"
    echo "   The app will test the connection when you add the storage source"
    echo ""
fi

echo "✅ Setup complete! You can now:"
echo "   1. Start the app: npm run tauri:dev"
echo "   2. Click '+' to add storage"
echo "   3. Select 'AWS S3'"
echo "   4. Enter bucket: $URSLY_S3_TEST_BUCKET"
echo "   5. Region: $AWS_REGION"
echo ""
echo "💡 Tip: The app will automatically use the AWS credentials from environment variables"

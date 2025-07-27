#!/bin/bash

# Secure deployment script for OpenAuth service
# This script deploys the auth service with KV namespace IDs from environment variables
# instead of hardcoding them in the configuration

set -e

# Check if environment variables are set
if [[ -f ".env.local" ]]; then
    source .env.local
    echo "✅ Loaded environment variables from .env.local"
else
    echo "❌ .env.local file not found. Please create it with KV namespace IDs."
    echo "Required variables:"
    echo "  KV_AUTH_STORAGE_ID=your_production_namespace_id"
    echo "  KV_AUTH_STORAGE_PREVIEW_ID=your_preview_namespace_id"
    exit 1
fi

# Validate required environment variables
if [[ -z "$KV_AUTH_STORAGE_ID" ]] || [[ -z "$KV_AUTH_STORAGE_PREVIEW_ID" ]]; then
    echo "❌ Missing required environment variables:"
    echo "  KV_AUTH_STORAGE_ID: ${KV_AUTH_STORAGE_ID:-'NOT SET'}"
    echo "  KV_AUTH_STORAGE_PREVIEW_ID: ${KV_AUTH_STORAGE_PREVIEW_ID:-'NOT SET'}"
    exit 1
fi

echo "🚀 Deploying auth service with secure KV namespace configuration..."

# Create temporary wrangler configuration with KV namespaces
cat > wrangler.temp.jsonc << EOF
{
  "\$schema": "node_modules/wrangler/config-schema.json",
  "name": "auth",
  "main": "src/index.ts",
  "compatibility_date": "2025-07-26",
  "compatibility_flags": [
    "global_fetch_strictly_public",
    "nodejs_compat"
  ],
  "assets": {
    "directory": "./public"
  },
  "observability": {
    "enabled": true
  },
  "kv_namespaces": [
    {
      "binding": "AUTH_STORAGE",
      "id": "$KV_AUTH_STORAGE_ID",
      "preview_id": "$KV_AUTH_STORAGE_PREVIEW_ID"
    }
  ]
}
EOF

# Deploy using the temporary configuration
echo "📦 Deploying to production..."
wrangler deploy --config wrangler.temp.jsonc

# Clean up temporary file
rm wrangler.temp.jsonc

echo "✅ Deployment completed successfully!"
echo "🔗 Your auth service is available at: https://auth.openagents.com"
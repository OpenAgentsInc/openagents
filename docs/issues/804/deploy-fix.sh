#!/bin/bash
# Deploy script for the WebSocket connection fix

# Print header
echo "=================================================="
echo "  Deploying WebSocket Connection Fix for Issue 804"
echo "=================================================="

# Navigate to the agents package directory
cd /Users/christopherdavid/code/openagents/packages/agents || { echo "❌ Failed to navigate to agents directory"; exit 1; }

echo "📂 Current directory: $(pwd)"

# Check if the development environment has the required dependencies
echo "🔍 Checking environment..."
if ! command -v wrangler &> /dev/null; then
    echo "❌ Wrangler is not installed. Please install it with 'npm install -g wrangler'"
    exit 1
fi

# Check if logged into Cloudflare
echo "🔑 Verifying Cloudflare authentication..."
wrangler whoami > /dev/null 2>&1 || { echo "❌ Not logged into Cloudflare. Please run 'wrangler login' first"; exit 1; }

# Build the worker
echo "🛠️  Building the worker..."
yarn build || { echo "❌ Build failed"; exit 1; }

# Deploy the worker
echo "🚀 Deploying the worker..."
wrangler deploy || { echo "❌ Deployment failed"; exit 1; }

# Check if the OpenRouter API key is set
echo "🔑 Checking if OpenRouter API key is set..."
wrangler secret get OPENROUTER_API_KEY > /dev/null 2>&1

if [ $? -eq 0 ]; then
    echo "✅ OpenRouter API key is already set"
else
    echo "⚠️  OpenRouter API key is not set. Please set it with:"
    echo "wrangler secret put OPENROUTER_API_KEY"
fi

echo ""
echo "✅ Deployment completed successfully"
echo "🌐 The WebSocket connection fix is now live"
echo ""
echo "To verify the fix:"
echo "1. Open the application in your browser"
echo "2. Connect to the CoderAgent"
echo "3. Check for successful WebSocket connection in browser devtools"
echo ""
echo "For more details, see: /docs/issues/804/websocket-connection-fix.md"
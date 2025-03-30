#!/bin/bash

# Deploy the WebSocket connection fix for Issue #804
# This script deploys the updated CoderAgent implementation with
# the case sensitivity fix for Cloudflare Agents integration

# Navigate to the agents package
cd /Users/christopherdavid/code/openagents/packages/agents

# Verify TypeScript compilation
echo "Verifying TypeScript compilation..."
npx tsc --noEmit

# If TypeScript compilation succeeds, deploy the worker
if [ $? -eq 0 ]; then
  echo "TypeScript compilation successful, deploying worker..."
  wrangler deploy
else
  echo "TypeScript compilation failed, fix errors before deploying."
  exit 1
fi

# Verify deployment
if [ $? -eq 0 ]; then
  echo "✅ Deployment successful!"
  echo "WebSocket connections should now work correctly."
  echo "Test URL: wss://agents.openagents.com/agents/coderagent/default"
else
  echo "❌ Deployment failed, check error messages above."
  exit 1
fi
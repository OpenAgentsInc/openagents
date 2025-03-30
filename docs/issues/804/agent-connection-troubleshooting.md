# Troubleshooting WebSocket Connections to Cloudflare Agents

This document provides guidance for troubleshooting WebSocket connection issues with Cloudflare Agents.

## Common Issues and Solutions

### 1. "URL does not match any server namespace" Error

**Error Message:**
```
The url https://agents.openagents.com/agents/coderagent/default does not match any server namespace.
```

**Possible Causes:**
- Case sensitivity mismatch between binding name and URL
- Missing or incorrectly configured Durable Object binding
- Incorrect URL pattern

**Solutions:**
1. Ensure binding name in wrangler.jsonc is lowercase to match client-side naming:
   ```json
   "name": "coderagent",  // Not "CoderAgent"
   ```
2. Verify that the binding exists and is correctly configured
3. Check that URL patterns match the expected format: `/agents/{agent-name}/{instance-name}`

### 2. "Missing namespace or room headers" Error

**Error Message:**
```
Missing namespace or room headers when connecting to CoderAgent.
```

**Possible Causes:**
- Attempting to access Durable Object directly without proper routing
- Manually adding incorrect headers
- Not using routeAgentRequest for routing

**Solutions:**
1. Use routeAgentRequest without any custom header manipulation
2. Simplify your server.ts implementation to match the example app
3. Don't attempt direct Durable Object access for WebSocket connections

### 3. WebSocket Connection Failures with Code 1006

**Error Message:**
```
Connection to wss://agents.openagents.com/agents/coderagent/default closed with code 1006 (Abnormal closure)
```

**Possible Causes:**
- Server-side error handling issue
- CORS configuration problem
- Authorization issues

**Solutions:**
1. Check server logs for errors during the WebSocket handshake
2. Ensure CORS is properly configured for WebSocket connections
3. Verify that all required API keys are set (e.g., OPENROUTER_API_KEY)

## Diagnostic Steps

When troubleshooting WebSocket connection issues:

1. **Check Client Console Logs:**
   - Look for WebSocket connection attempts
   - Note any error messages or status codes

2. **Check Server Logs:**
   - Look for request handling information
   - Note any errors during WebSocket upgrade

3. **Verify Configuration:**
   - Ensure wrangler.jsonc has the correct binding name (lowercase)
   - Verify that the class_name matches the exported class
   - Check that the server.ts file follows the example app pattern

4. **Test with Simple Implementation:**
   - Simplify to the most basic implementation
   - Use the exact pattern from the Cloudflare Agents starter app

## Helpful Commands

```bash
# Deploy worker and check for errors
wrangler deploy

# Test WebSocket connection from command line
websocat wss://agents.openagents.com/agents/coderagent/default

# Check for TypeScript errors
npx tsc --noEmit
```

## Example of Working Configuration

**wrangler.jsonc:**
```jsonc
"durable_objects": {
  "bindings": [
    {
      "name": "coderagent",
      "class_name": "CoderAgent"
    }
  ]
},
```

**server.ts:**
```typescript
import { routeAgentRequest } from "agents";
import { CoderAgent } from "./coder-agent";

export { CoderAgent };

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    return (await routeAgentRequest(request, env)) || new Response("Not found", { status: 404 });
  },
};
```
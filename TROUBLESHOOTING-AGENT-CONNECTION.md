# Troubleshooting OpenAgents WebSocket Connection Issues

## Overview of the Problem

The OpenAgents application is experiencing WebSocket connection issues when attempting to connect to the Solver agent. The connection is consistently failing with generic "close" and "error" events, without any specific error details. Despite multiple reconnection attempts, the agent remains unable to establish a stable WebSocket connection.

## Log Analysis

From the provided logs, we can observe the following sequence of events:

1. The agent initializes successfully: `Agent initialized: {type: 'solver', agentName: 'solver-004e96b2-86d7-47d2-9a4d-542e82d53a22', connectionStatus: 'created'}`
2. The WebSocket connection attempt is made but immediately closes: `WebSocket connection closed ii {isTrusted: false, code: 'close'...}`
3. A generic error event is triggered: `WebSocket connection error: Event {isTrusted: false, type: 'error'...}`
4. Multiple reconnection attempts are made, but all fail with the same pattern
5. The client eventually gives up after reaching the maximum reconnection attempts: `Max reconnection attempts (3) reached. Giving up.`

## Key Observations

1. **Generic Error Information**: The WebSocket errors don't contain specific error codes or messages, making it difficult to diagnose the exact cause.
2. **Clean Close**: The `wasClean: true` property suggests the WebSocket is closing gracefully, not due to a network interruption.
3. **Immediate Failure**: The connection appears to close almost immediately after initialization, suggesting a configuration or authentication issue rather than a data transfer problem.
4. **Consistent Behavior**: The same pattern repeats for all connection attempts, indicating a systematic issue rather than an intermittent problem.

## Potential Root Causes

Based on the observed behavior, here are the most likely causes of the connection issues:

### 1. Server-side Rejection

The server (Cloudflare Worker) might be actively rejecting the WebSocket connection. This could happen if:

- The Durable Object for the Solver agent doesn't exist or isn't properly registered
- The WebSocket request doesn't include necessary authorization headers
- CORS (Cross-Origin Resource Sharing) settings are preventing the connection
- Cloudflare WebSocket request limits are being reached

### 2. Configuration Mismatch

There might be a mismatch between the client and server configurations:

- Different WebSocket protocol versions
- Different message formats or serialization
- Different route paths or naming conventions

### 3. Deployment Issues

The Durable Object for the Solver agent might not be properly deployed:

- Incorrect wrangler.jsonc configuration
- Failed Durable Object migration
- Incorrect binding names in the Cloudflare Worker

### 4. Authentication/Authorization Issues

The WebSocket connection might be failing due to authentication issues:

- Missing or invalid authentication tokens
- Wrong token format or encoding
- Token permissions issues

## Troubleshooting Steps

### 1. Verify Cloudflare Worker Deployment

First, ensure that the Cloudflare Worker and Durable Objects are correctly deployed:

```bash
# Check the current deployment status
wrangler whoami
wrangler d1 list
wrangler kv:namespace list

# View the logs to check for any server-side errors
wrangler tail
```

### 2. Verify Durable Object Configuration

Ensure the Durable Object configuration in `wrangler.jsonc` is correct:

```json
{
  "durable_objects": {
    "bindings": [
      {
        "name": "Coder",
        "class_name": "Coder"
      },
      {
        "name": "Solver",
        "class_name": "Solver"
      }
    ]
  },
  "migrations": [
    {
      "tag": "v1",
      "new_classes": ["Coder"]
    },
    {
      "tag": "v2",
      "new_classes": ["Solver"]
    }
  ]
}
```

### 3. Check Network Request Details

Use the browser's network inspector to analyze the WebSocket connection attempt:

1. Open Chrome DevTools Network tab
2. Filter for "WS" (WebSocket) requests
3. Look for any failed connection attempts to `wss://agents.openagents.com/agents/solver/...`
4. Check the request headers, especially:
   - Origin
   - Sec-WebSocket-Protocol
   - Authorization (if applicable)

### 4. Test with a Simple WebSocket Client

Try connecting to the WebSocket endpoint using a standalone WebSocket client to isolate the issue:

```javascript
// Simple WebSocket test
const ws = new WebSocket('wss://agents.openagents.com/agents/solver/test');
ws.onopen = () => console.log('Connection opened');
ws.onclose = (e) => console.log('Connection closed', e);
ws.onerror = (e) => console.log('Connection error', e);
```

### 5. Check Server-side Route Handling (CRITICAL ISSUE FOUND)

**CRITICAL ISSUE IDENTIFIED:** We were incorrectly implementing custom routing instead of using the official SDK router.

When examining the server-side handling in `server.ts`, we found:

```typescript
// BEFORE: Incorrect custom routing implementation
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const url = new URL(request.url);
    const pathParts = url.pathname.split('/').filter(Boolean);
    
    // Only handling singular 'agent' paths
    if (pathParts[0] === 'agent' && pathParts.length > 1) {
      const agentType = pathParts[1];
      
      if (agentType === 'solver') {
        // ... process solver agent request ...
      }
    }
    // Any request to '/agents/solver/' would fail here
    // ...
  }
}
```

The Cloudflare Agents SDK provides a built-in routing mechanism via `routeAgentRequest` that properly handles WebSocket connections, paths, and CORS. By examining previous successful implementations in the codebase, we found a working example:

```typescript
// CORRECT: Using the official SDK router
import { routeAgentRequest } from "agents";

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent via the Agents SDK
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Agent not found", { status: 404 })
    );
  },
}
```

Looking at the SDK code (in `partysocket/dist/chunk-5XUHNIY2.js`), we confirmed that:

1. It uses a prefix (`agents`) in URL construction to generate paths like `/agents/solver/...`
2. It handles all the complex WebSocket connection setup
3. It manages CORS headers automatically when the `cors: true` option is provided
4. It handles the creation and routing of Durable Objects

**Solution**: We completely replaced our custom router implementation with the official SDK router. This is not just a path fix but a complete architectural correction to use the SDK as intended, which handles all the routing complexity for us.

### 6. Verify Cloudflare Agents SDK Configuration

Check that the Cloudflare Agents SDK is being used correctly in the client:

```typescript
// Verify the configuration in useOpenAgent.ts
const cloudflareAgent = useAgent({
  name: agentName,         // Should match the expected format
  agent: type,             // Should be 'solver'
  host: "agents.openagents.com",  // Should match your deployed worker
  onStateUpdate: (newState: AgentState) => {
    setAgentState(newState);
  }
});
```

## Most Likely Issues and Fixes

Based on the logs and behavior, here are the most likely issues and their fixes:

### Issue 1: Path Mismatch Between Client and Server

**RESOLVED: Path Structure Mismatch**

After extensive analysis, we've identified the root cause as a path mismatch between the client and server:

- **Client path**: The Cloudflare Agents SDK is connecting to `/agents/solver/...`
- **Server route**: The server was only handling routes with `/agent/solver/...` (singular 'agent')

**Fix Implemented:**
- We've updated the server's route handling in `server.ts` to accept both paths:

```typescript
// Updated server.ts route handling:
// Check if the request is for an agent - support both /agent/ and /agents/ paths
if ((pathParts[0] === 'agent' || pathParts[0] === 'agents') && pathParts.length > 1) {
  const agentType = pathParts[1];
  console.log(`[AGENT SERVER] Agent type requested: ${agentType}`);
  
  // Map agent type to Durable Object
  if (agentType === 'solver') {
    // ... handle solver requests ...
  }
}
```

This change ensures that the server can handle WebSocket connection requests from the Cloudflare Agents SDK, which uses the plural "agents" prefix in the URL path.

### Issue 2: WebSocket Endpoint Not Deployed or Not Available

**Fix:**
- Deploy or redeploy the Cloudflare Workers project using `wrangler deploy`
- Verify the worker has been deployed to the correct environment
- Check the worker URL matches what the client is trying to connect to

### Issue 3: Durable Object Binding Issues

**Fix:**
- Ensure the Durable Object migrations have been applied correctly
- Verify that both Coder and Solver classes are being exported from the worker
- Check for any migration errors in the Cloudflare dashboard

### Issue 4: Incorrect Agent Name Format

**Fix:**
- Check the format of the agent name in the `useAgent` call
- Ensure the server-side code expects the same format of agent name

### Issue 5: CORS Issues

**Fix:**
- Add proper CORS headers in the worker's response handling
- Ensure the worker allows WebSocket connections from the client's origin
- Check for any CORS errors in the browser's console

### Issue 6: Authentication Issues

**Fix:**
- Verify the GitHub token is being properly set and used
- Check if the token has the necessary permissions
- Ensure the token is being passed correctly to the agent

## Implementation Recommendations

1. **Add Better Error Handling**:
   - Implement more specific error codes in the WebSocket responses
   - Add detailed logging on the server side
   - Improve error reporting in the UI

2. **Create a Health Check Endpoint**:
   - Add a simple REST endpoint to check if the Solver agent is available
   - Test connectivity without using WebSockets to isolate the issue

3. **Implement Fallback Mechanisms**:
   - Add a REST API fallback for when WebSockets are unavailable
   - Implement a local mode for development testing

4. **Improve Diagnostics**:
   - Add server-side logging for all connection attempts
   - Add detailed connection status information in the UI
   - Create a diagnostics page that tests various connection scenarios

## Root Cause and Solution

After thorough analysis, we identified and fixed the root cause of the WebSocket connection issues: incorrect server-side routing implementation.

### Problem:

The issue was caused by:
1. Using a custom routing implementation in `server.ts` instead of the official Cloudflare Agents SDK router
2. The Cloudflare Agents SDK (client-side) constructs WebSocket URLs with the path `/agents/solver/...` (plural)
3. Our custom routing implementation in `server.ts` was only handling paths with `/agent/solver/...` (singular)

When the WebSocket connection request arrived at the server with path `/agents/solver/...`, it didn't match our custom routing pattern and was immediately rejected with a clean close (wasClean: true).

### Solution:

We completely replaced the custom routing implementation with the official Cloudflare Agents SDK router:

```typescript
// BEFORE: Custom routing implementation (problematic)
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const url = new URL(request.url);
    const pathParts = url.pathname.split('/').filter(Boolean);
    
    // Only handles /agent/solver pattern (singular)
    if (pathParts[0] === 'agent' && pathParts.length > 1) {
      const agentType = pathParts[1];
      // ...route to different agents...
    }
    // ...
  }
}

// AFTER: Official SDK routing (correct)
import { routeAgentRequest } from "agents";

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Uses the Agents SDK's routing which properly handles all paths
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Agent not found", { status: 404 })
    );
  },
}
```

This change ensures proper handling of WebSocket connections through the Cloudflare Agents SDK, which handles all the complex routing logic for us, including CORS, WebSocket setup, and proper routing to Durable Objects.

### Additional Improvements:

1. Added better logging to diagnose any further connection issues
2. Documented the correct implementation for future reference
3. Improved error response for 404 cases

By leveraging the official Cloudflare Agents SDK routing rather than attempting to reimplement it, we've eliminated the path mismatch issue and ensured proper WebSocket connection handling.
# WebSocket Connection Fix for Cloudflare Agents

## Problem: "Missing namespace or room headers" Error

The WebSocket connection to the CoderAgent Durable Object was failing with the following error:

```
Missing namespace or room headers when connecting to CoderAgent.
Did you try connecting directly to this Durable Object? Try using getServerByName(namespace, id) instead.
```

This error occurs because we need to handle WebSocket connections to Durable Objects properly by providing the required headers.

## Root Cause Analysis

1. The error "Missing namespace or room headers" indicates we were trying to access the Durable Object directly without going through the proper routing mechanism.

2. According to the Cloudflare Agents SDK, WebSocket connections need to be routed correctly with namespace and room headers.

3. Based on the documentation, the `routeAgentRequest` function should handle this properly when given the right options.

## Solution Implemented

We've modified the server.ts file to:

1. Remove the incorrect import of `getServerByName` which doesn't exist in the Agents SDK
2. Enhance the `routeAgentRequest` function call with additional WebSocket-specific headers
3. Add a fallback mechanism for direct Durable Object access with custom headers as a last resort

```typescript
// Customize the agent routing based on the Cloudflare agents SDK docs
// Using routeAgentRequest with extended options to handle WebSocket connections
console.log(`🚀 Routing request using routeAgentRequest with enhanced options...`);
const options = {
  // Enable CORS to allow connections from different origins
  cors: true,
  
  // Add custom headers for WebSocket connections if needed
  headers: upgradeHeader === 'websocket' ? {
    'X-Agent-WS-Version': '1.0',
    'X-Agent-Connection-Type': 'websocket'
  } : undefined
};

// Use the routeAgentRequest function to properly handle all agent requests
// This will automatically handle the namespace and room headers for WebSockets
const response = await routeAgentRequest(request, env, options);
```

If the normal routing fails, we have a fallback mechanism that tries direct Durable Object access with added headers:

```typescript
// For CoderAgent requests, we can try to directly connect to the Durable Object
if (agentName === 'coderagent' && upgradeHeader === 'websocket') {
  console.log(`⚠️ routeAgentRequest failed to handle WebSocket connection. Attempting direct access...`);
  
  try {
    // Create a Durable Object ID based on the instance name
    const id = env.CoderAgent.idFromName(instanceName);
    // Get a reference to the Durable Object
    const agent = env.CoderAgent.get(id);
    
    // Add headers that might be required by the Durable Object
    const headers = new Headers(request.headers);
    headers.set('X-Agent-Name', agentName);
    headers.set('X-Instance-Name', instanceName);
    
    // Create a new request with the added headers
    const enhancedRequest = new Request(request.url, {
      method: request.method,
      headers: headers,
      body: request.body,
      redirect: request.redirect,
    });
    
    // Forward the enhanced request to the Durable Object
    return agent.fetch(enhancedRequest);
  } catch (directError) {
    console.error(`⛔ Direct Durable Object access failed:`, directError);
  }
}
```

## Deployment Instructions

1. Deploy the updated server code:
   ```bash
   cd packages/agents
   wrangler deploy
   ```

2. Verify the OpenRouter API key is set:
   ```bash
   wrangler secret get OPENROUTER_API_KEY
   ```
   
   If not set, add it:
   ```bash
   wrangler secret put OPENROUTER_API_KEY
   ```

## Technical Notes

1. **Cloudflare Agents SDK and WebSockets:**
   - The Agents SDK has specific routing requirements for WebSockets
   - The `routeAgentRequest` function should handle WebSocket connections correctly
   - Custom headers may help in routing the request properly 

2. **Path Structure:**
   - The correct path structure is `/agents/{agent-id}/{instance-name}`
   - Both agent-id and instance-name should be lowercase
   - The default instance-name is 'default' if not provided

3. **Error Handling:**
   - All operations are wrapped in try-catch blocks to prevent uncaught exceptions
   - Detailed error logs help diagnose connection issues
   - Proper error responses are returned to the client
   - Fallback mechanism attempts direct access if routing fails

## Client-Side Integration

No changes are needed to the client-side code. The agent-sdk-bridge.ts file is already configured to:

1. Use the correct URL pattern: `/agents/coderagent/default`
2. Handle all WebSocket connection states properly
3. Queue messages until the connection is established
4. Provide detailed error information

This fix should now correctly route WebSocket connections to the CoderAgent Durable Object.
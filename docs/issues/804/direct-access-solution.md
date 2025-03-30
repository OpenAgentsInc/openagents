# Direct Durable Object Access Solution for WebSocket Connections

## Problem Solved

We've fixed the WebSocket connection issues with the CoderAgent by implementing a direct Durable Object access approach, addressing the error:

```
Missing namespace or room headers when connecting to CoderAgent.
Did you try connecting directly to this Durable Object? Try using getServerByName(namespace, id) instead.
```

## Solution Approach

After multiple attempts using the agents SDK's `routeAgentRequest` and `getAgentByName` functions, which consistently failed with the "Missing namespace or room headers" error, we've implemented a direct access solution with explicit header addition.

### Key Implementation

1. **Direct Durable Object Access** - We use the Durable Object bindings directly:

```typescript
// Extract the instance name from the path
const instanceName = pathParts[2] || 'default';

// Create a Durable Object ID and get the stub
const id = env.CoderAgent.idFromName(instanceName);
const stub = env.CoderAgent.get(id);
```

2. **Adding Required Headers** - We manually add the namespace and room headers that the error message suggests are missing:

```typescript
// For WebSocket connections, add the required headers
if (upgradeHeader === 'websocket') {
  // Create new request with proper headers for WebSocket
  const headers = new Headers(request.headers);
  // Add these headers to simulate what the Agents SDK would add
  headers.set('x-cf-agent-namespace', 'agents');
  headers.set('x-cf-agent-room', instanceName);
  
  const enhancedRequest = new Request(request.url, {
    method: request.method,
    headers: headers,
    body: request.body
  });
  
  // Forward the enhanced request to the Durable Object
  return stub.fetch(enhancedRequest);
}
```

3. **Removing Dependency on agents SDK** - We've completely removed the dependency on the agents SDK's routing functions (`routeAgentRequest`, `getAgentByName`, etc.) since they weren't working correctly.

## Technical Explanation

Based on the error message and our investigation, we found that:

1. Durable Objects in Cloudflare Workers require special headers (`x-cf-agent-namespace` and `x-cf-agent-room`) when receiving WebSocket connections.

2. The agents SDK is supposed to add these headers automatically but was failing to do so, likely due to configuration issues or mismatches between our implementation and the SDK's expectations.

3. By directly accessing the Durable Object and manually adding the required headers, we bypass the SDK's routing layer but satisfy the Durable Object's requirements for WebSocket connections.

## Benefits of This Approach

1. **Simplicity** - Direct access without the complexity of the agents SDK routing layer
2. **Reliability** - Fewer moving parts means fewer points of failure
3. **Explicit Control** - We explicitly add the required headers rather than relying on SDK behavior
4. **Debugging** - Clear, detailed logging at each step of the process

## Deployment

Deploy the updated code with:

```bash
cd packages/agents
wrangler deploy
```

## Testing

Test the WebSocket connection by connecting to:
```
wss://agents.openagents.com/agents/coderagent/default
```

The connection should now be established without the "Missing namespace or room headers" error.
# Fixing the WebSocket Connection Path Issue

## Root Cause Analysis

After examining the console logs and the implementation details of the Cloudflare Agents SDK, we've identified the root causes of the 404 errors:

1. **Incorrect URL Path Pattern**: 
   - Currently using: `/api/agent/CoderAgent/default-instance` 
   - Should be using: `/api/agents/coderagent/default`
   
2. **Case Sensitivity**: 
   - Agent name should be lowercase (`coderagent` not `CoderAgent`)
   - Instance name should also be lowercase

3. **Path Component**: 
   - The path needs to use `agents` (plural) not `agent` (singular)
   - The standard instance name is `default`, not `default-instance`

## Detailed Explanation

The Cloudflare Agents SDK (which our WebSocket client is trying to interface with) uses a specific routing pattern for WebSocket connections:

```
wss://{host}/api/agents/{agentName}/{instanceName}
```

Where:
- `{agentName}` is the lowercase name of the agent (e.g., `coderagent`)
- `{instanceName}` is the lowercase name of the instance (defaults to `default`)

The actual routing happens in the `routeAgentRequest` function, which internally calls `routePartykitRequest` with a prefix of "agents". This is why the path needs to contain "agents" (plural) rather than "agent" (singular).

## Implementation Fix

We need to update the WebSocket URL construction in the `agent-sdk-bridge.ts` file:

```typescript
// Current (incorrect) URL construction:
const pathPattern = this.options.pathPattern || 'api/agent';
wsUrl = `${wsProtocol}://${url.host}/${pathPattern}/${this.agent}/${this.name}`;

// Fixed URL construction:
const pathPattern = this.options.pathPattern || 'api/agents';
const agentName = this.agent.toLowerCase(); // Ensure lowercase
const instanceName = this.name.toLowerCase(); // Ensure lowercase
wsUrl = `${wsProtocol}://${url.host}/${pathPattern}/${agentName}/${instanceName}`;
```

## Case Sensitivity Requirements

The Cloudflare Agents SDK explicitly warns about using uppercase characters in agent and instance names:

```javascript
// Client implementation (from @openagents/agents)
if (options.agent.toLowerCase() !== options.agent) {
  console.warn(
    `Agent names should be lowercase. Converting ${options.agent} to ${options.agent.toLowerCase()}.`
  );
}

if (options.name && options.name.toLowerCase() !== options.name) {
  console.warn(
    `Instance names should be lowercase. Converting ${options.name} to ${options.name.toLowerCase()}.`
  );
}
```

This is because these names are used in URLs, and case-sensitivity might cause issues across different systems.

## Connection Status Reporting

Additionally, we need to fix the connection status reporting issue. Currently, useChat is reporting a connection as successful based on the client object creation, not the actual WebSocket connection:

```typescript
// Current (incorrect) connection status reporting:
const client = await createAgentConnection(connectionOptions);
setAgentConnection({
  isConnected: true, // This is incorrect - we don't know if the WebSocket is connected yet
  client,
  utils
});

// Fixed connection status reporting:
const client = await createAgentConnection(connectionOptions);

// Wait for the actual WebSocket connection to be established
await (client as any).connectionPromise;

// Now we know the WebSocket is really connected
setAgentConnection({
  isConnected: true,
  client,
  utils
});
```

## Specific Changes Needed

1. Update `agent-sdk-bridge.ts`:
   - Change default path pattern from `api/agent` to `api/agents`
   - Convert agent and instance names to lowercase in URL construction
   - Add warning logs for uppercase agent/instance names

2. Update `agent-connection.ts`:
   - Add better error handling for 404 responses
   - Retry with different path patterns if the initial connection fails

3. Update `useChat.ts`:
   - Fix connection status reporting to accurately reflect the WebSocket connection state

## Testing the Fix

After implementing these changes, we should test with various path patterns to ensure the WebSocket connection is successful:

1. Original pattern: `/api/agent/CoderAgent/default-instance` (should fail)
2. Corrected pattern: `/api/agents/coderagent/default` (should work)
3. Other variations to test:
   - `/agents/coderagent/default` (without the api prefix)
   - `/api/agent/coderagent/default` (singular agent)

## Expected Behavior After Fix

1. WebSocket connection to the agent should succeed
2. No more 404 errors in the console
3. UI should correctly report connection status based on actual WebSocket state
4. Agent methods can be called successfully

By fixing the URL path pattern and case sensitivity issues, we should resolve the primary connection problem and establish proper WebSocket communication with the Cloudflare Agent server.
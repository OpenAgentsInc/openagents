# WebSocket Connection Testing Guide

This document provides guidance on testing and validating WebSocket connections to Cloudflare Agents.

## Connection Configuration

The correct WebSocket URL for connecting to Cloudflare Agents follows this pattern:

```
wss://agents.openagents.com/api/agents/{agentName}/{instanceName}
```

Where:
- `{agentName}` is the lowercase name of the agent (e.g., `coderagent`)
- `{instanceName}` is the lowercase name of the instance (defaults to `default`)

## Manual Testing with Browser Developer Tools

You can test WebSocket connections directly in your browser's developer tools console:

```javascript
// Test with the correct path pattern
const testCorrectSocket = new WebSocket('wss://agents.openagents.com/api/agents/coderagent/default');

testCorrectSocket.onopen = () => console.log('✅ Correct WebSocket connected!');
testCorrectSocket.onerror = (e) => console.error('❌ Correct WebSocket error:', e);
testCorrectSocket.onclose = (e) => console.log('Correct WebSocket closed:', e.code, e.reason);

// Test with the incorrect path pattern (should fail)
const testIncorrectSocket = new WebSocket('wss://agents.openagents.com/api/agent/CoderAgent/default-instance');

testIncorrectSocket.onopen = () => console.log('✅ Incorrect WebSocket connected!');
testIncorrectSocket.onerror = (e) => console.error('❌ Incorrect WebSocket error:', e);
testIncorrectSocket.onclose = (e) => console.log('Incorrect WebSocket closed:', e.code, e.reason);
```

## Testing in the AgentChatTest Component

The `AgentChatTest` component should be configured with the correct connection parameters:

```typescript
<AgentChatTest
  agentId="coderagent" // lowercase!
  agentName="default"  // lowercase!
  pathPattern="api/agents" // plural!
/>
```

## Connection Validation

The connection status in the UI should accurately reflect the WebSocket connection state. The updated implementation now:

1. Creates a client object but reports `isConnected: false` initially
2. Waits for the actual WebSocket connection to be established
3. Only sets `isConnected: true` once the WebSocket is in the OPEN state
4. Properly reports connection failures

## WebSocket State Debugging

You can check the WebSocket state at any time using:

```javascript
// Get the WebSocket clients from the application
const clientObjects = Array.from(document.querySelectorAll('div'))
  .filter(el => el.__reactFiber$)
  .map(el => {
    try {
      let fiber = el.__reactFiber$;
      while (fiber && !fiber.memoizedState?.memoizedState?.agentConnection?.client) {
        fiber = fiber.return;
      }
      return fiber?.memoizedState?.memoizedState?.agentConnection?.client;
    } catch (e) {
      return null;
    }
  })
  .filter(Boolean);

// Log all WebSocket clients and their connection states
clientObjects.forEach((client, i) => {
  const socket = client.socket;
  const states = ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'];
  console.log(`Client ${i}:`, {
    agent: client.agent,
    name: client.name,
    socketState: socket ? states[socket.readyState] : 'NO_SOCKET',
    connected: client.connected,
    connecting: client.connecting,
    wsUrl: new URL(socket?.url || 'about:blank').pathname
  });
});
```

## Common Issues and Solutions

1. **404 Errors**: If you see a 404 error, check:
   - Is the URL path correct? Should be `/api/agents/` (plural)
   - Is the agent name lowercase? Case matters in URLs
   - Is the instance name lowercase? Case matters in URLs

2. **Connection Reporting Mismatch**: If the UI shows "connected" but commands fail:
   - Check if the client object exists but the WebSocket connection failed
   - Look for errors in the console log with the WebSocket error details
   - Verify the socket state is actually `OPEN` using the debugging code above

3. **Authentication Issues**: If you get 401/403 errors:
   - Verify the authentication token is being passed correctly
   - Check if the token has the right permissions
   - Check server logs for authentication failures

## Integration Tests

Add integration tests that verify the WebSocket connection is properly established:

```typescript
// Test successful connection with correct parameters
test('Connects to agent with correct parameters', async () => {
  const { result, waitFor } = renderHook(() => useChat({
    agentId: 'coderagent',
    agentName: 'default',
    pathPattern: 'api/agents'
  }));
  
  await waitFor(() => {
    expect(result.current.agentConnection.isConnected).toBe(true);
  });
});

// Test failed connection with incorrect parameters
test('Fails to connect with incorrect parameters', async () => {
  const { result, waitFor } = renderHook(() => useChat({
    agentId: 'CoderAgent', // uppercase will fail
    agentName: 'default-instance',
    pathPattern: 'api/agent' // singular will fail
  }));
  
  await waitFor(() => {
    expect(result.current.agentConnection.isConnected).toBe(false);
  });
});
```

## Server-Side Logging

If you have access to the server logs, look for:

1. WebSocket connection attempts
2. 404 errors indicating bad URL paths
3. Authentication failures
4. WebSocket protocol errors

Common log patterns for WebSocket connection failures:
```
[2025-03-29 15:48:23] WebSocket connection to /api/agent/CoderAgent/default-instance - 404 Not Found
[2025-03-29 15:48:24] WebSocket connection to /api/agents/coderagent/default - 101 Switching Protocols (success)
```

## Recommended Configuration

For most reliable connections, use:

```typescript
const connectionOptions = {
  agentId: 'coderagent', // lowercase!
  agentName: 'default',  // simple, lowercase name
  pathPattern: 'api/agents', // plural!
  serverUrl: 'https://agents.openagents.com'
};
```

This configuration follows the expected URL pattern and naming conventions for the Cloudflare Agents SDK.
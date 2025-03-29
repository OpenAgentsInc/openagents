# Agent Connection Troubleshooting Guide

This document provides guidance on troubleshooting WebSocket connections to Cloudflare Agents in the OpenAgents system.

## Common Connection Issues

When connecting to Cloudflare Agents, you might encounter the following issues:

1. **404 Not Found Errors**
   - The WebSocket endpoint path may be incorrect
   - The agent name may be incorrectly formatted
   - The server might be running at a different URL

2. **Connection Status Mismatch**
   - The UI shows "connected" but the WebSocket is not actually connected
   - Commands fail with "connection not established" errors

3. **Authentication Failures**
   - Missing or invalid authentication token

## Connection Path Patterns

The agent-sdk-bridge will now automatically try multiple URL patterns to find the correct endpoint:

```typescript
const possiblePatterns = [
  'api/agent',  // Singular (original pattern)
  'api/agents', // Plural (SDK docs pattern)
  'agents',     // Without api prefix
  '',           // Direct path
  'ws',         // WebSocket-specific
  'worker',     // Worker-specific endpoint
  'agent'       // Direct agent endpoint
];
```

Each pattern is combined with the agent name and instance name:
`wss://host/{pattern}/{agentName}/{instanceName}`

## Best Practices

1. **Use Lowercase Agent Names**
   - Always use lowercase agent names for compatibility
   - The system will automatically convert names to lowercase

2. **Use Simple Instance Names**
   - Use simple instance names like `default` instead of `default-instance`
   - Avoid special characters and spaces

3. **Don't Specify Path Pattern Unless Necessary**
   - Let the system try multiple patterns to find the working one
   - Only specify a path pattern if you know the exact endpoint structure

4. **Check Console Logs**
   - The system now logs detailed connection attempts and errors
   - Look for "Starting connection attempts with X possible URL patterns"
   - Check for "Connection to [URL] closed with code [code]" messages

## Monitoring Connection Status

The system now provides accurate connection status reporting:

```typescript
// In your component
const chat = useChat({
  agentId: 'coderagent',
  onAgentConnectionChange: (connected) => {
    console.log(`Connection status: ${connected ? 'connected' : 'disconnected'}`);
  }
});

// Check connection status
if (chat.agentConnection?.isConnected) {
  console.log('WebSocket is connected');
} else {
  console.log('WebSocket is not connected');
}
```

## Testing Connection

You can test WebSocket connections directly in your browser console:

```javascript
// Test different URL patterns
function testAgentConnections() {
  const host = 'agents.openagents.com';
  const agentName = 'coderagent';
  const instanceName = 'default';
  
  const patterns = [
    'api/agent',
    'api/agents',
    'agents',
    '',
    'ws',
    'worker',
    'agent'
  ];
  
  patterns.forEach((pattern, index) => {
    const path = pattern ? `${pattern}/` : '';
    const url = `wss://${host}/${path}${agentName}/${instanceName}`;
    
    console.log(`Testing pattern ${index + 1}: ${url}`);
    
    const socket = new WebSocket(url);
    
    socket.onopen = () => {
      console.log(`✅ CONNECTED using pattern: ${pattern}`);
      // Close this socket since we're just testing
      setTimeout(() => socket.close(), 1000);
    };
    
    socket.onerror = () => {
      console.log(`❌ FAILED using pattern: ${pattern}`);
    };
    
    socket.onclose = (event) => {
      console.log(`CLOSED pattern ${pattern}: code=${event.code}`);
    };
  });
}

// Run the test
testAgentConnections();
```

## Debugging Tips

1. **Check Server Logs**
   - If you have access to the Cloudflare Worker logs, check for WebSocket connection attempts
   - Look for 404 errors which indicate incorrect paths

2. **Verify Agent Name**
   - Make sure the agent name exactly matches the exported class on the server
   - Check case sensitivity (although the client converts to lowercase)

3. **Test Each Component Separately**
   - Test direct WebSocket connections using the browser console
   - Test agent client without the UI layer
   - Test basic functionality before adding complexity

4. **Check Network Tab**
   - In browser DevTools, check the Network tab for WebSocket connection attempts
   - Look for 101 (Switching Protocols) responses for successful connections
   - Check for 404, 401, or other error responses

By following these troubleshooting steps, you should be able to identify and resolve connection issues with Cloudflare Agents.
# Agent URL Pattern Analysis

Based on the continued 404 errors, we need to try different URL patterns. The logs show we've tried:

```
wss://agents.openagents.com/api/agents/coderagent/default-instance  // 404 error
```

## Possible URL Patterns to Try

We need to systematically test multiple URL patterns:

1. **Pattern Options**:
   - `/api/agents/{agent}/{instance}` (current, fails with 404)
   - `/api/agent/{agent}/{instance}` (original pattern)
   - `/agents/{agent}/{instance}` (without api prefix)
   - `/{agent}/{instance}` (direct path)
   - `/ws/{agent}/{instance}` (websocket-specific path)

2. **Case Options**:
   - All lowercase (current)
   - Original case

## Implementation Strategy

Let's modify the agent-sdk-bridge.ts to attempt multiple patterns in sequence:

```typescript
// Define array of possible path patterns to try
const possiblePatterns = [
  'api/agents', // Current (fails)
  'api/agent',  // Original 
  'agents',     // Without api prefix
  '',           // Direct path
  'ws'          // WebSocket-specific
];

// Try each pattern in sequence until one works
for (const pattern of possiblePatterns) {
  try {
    // Attempt connection with this pattern
    // If it succeeds, break the loop
  } catch (error) {
    // Log failure and continue to next pattern
  }
}
```

## Recommended Changes

1. **Update agent-sdk-bridge.ts to try multiple patterns**:
   - Implement a pattern fallback mechanism
   - Log success/failure for each pattern

2. **Fix AgentChatTest component to use lowercase by default**:
   - Ensure the component correctly converts to lowercase

3. **Document the correct pattern once discovered**:
   - Update all documentation with the working pattern

## Testing WebSocket Endpoints

Let's create a simple tool to test WebSocket endpoints directly:

```javascript
// Function to test a WebSocket connection
function testWebSocketEndpoint(url) {
  console.log(`Testing WebSocket connection to: ${url}`);
  
  const socket = new WebSocket(url);
  
  socket.onopen = () => {
    console.log(`✅ CONNECTED to ${url}`);
    socket.close(); // Close after successful connection
  };
  
  socket.onerror = (error) => {
    console.error(`❌ ERROR connecting to ${url}:`, error);
  };
  
  socket.onclose = (event) => {
    console.log(`CLOSED connection to ${url}: code=${event.code}, reason=${event.reason || 'none'}`);
  };
  
  return socket;
}

// Test different patterns
const baseUrl = 'wss://agents.openagents.com';
const agentName = 'coderagent';
const instanceName = 'default';

const patterns = [
  `/api/agents/${agentName}/${instanceName}`,
  `/api/agent/${agentName}/${instanceName}`,
  `/agents/${agentName}/${instanceName}`,
  `/${agentName}/${instanceName}`,
  `/ws/${agentName}/${instanceName}`
];

patterns.forEach(pattern => {
  testWebSocketEndpoint(`${baseUrl}${pattern}`);
});
```

This approach will help us identify the correct endpoint pattern systematically.
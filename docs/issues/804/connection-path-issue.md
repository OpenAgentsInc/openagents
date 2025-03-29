# WebSocket Connection Path Issue

## Issue Analysis

Based on the console logs, we've identified a critical issue with the WebSocket connection to the Cloudflare Agent server:

```
agent-sdk-bridge.ts:102 Connecting to agent at wss://agents.openagents.com/api/agent/CoderAgent/default-instance
agent-sdk-bridge.ts:116 WebSocket connection to 'wss://agents.openagents.com/api/agent/CoderAgent/default-instance' failed: Error during WebSocket handshake: Unexpected response code: 404
```

The 404 error indicates that the server endpoint we're trying to reach doesn't exist.

## Key Observations

1. The UI is showing "connected" status even though the WebSocket connection is failing.
2. The connection attempts to connect to `wss://agents.openagents.com/api/agent/CoderAgent/default-instance`.
3. The server consistently responds with a 404 status code.
4. The reconnection attempts also consistently fail with the same error.
5. The WebSocket connection is correctly showing detailed error information but we're using the wrong endpoint.

## Core Issue

**The WebSocket endpoint path is incorrect.**

While our error handling is now robust and informative, we're still attempting to connect to a non-existent endpoint. We need to determine the correct path pattern for the Cloudflare Worker that hosts the Agent.

## Suggested Fixes

1. **Server Inspection**: We need to determine the actual WebSocket endpoint supported by the Cloudflare Worker.

2. **Path Pattern Testing**: We should test different path patterns:
   - `/agents/:agent/:instance`
   - `/agent/:agent/:instance`
   - `/ws/agent/:agent/:instance`
   - `/:agent/:instance`

3. **Connection Status Accuracy**: Fix the UI to only show "connected" when the WebSocket is actually connected (readyState === WebSocket.OPEN), not just when the client object is created.

4. **Server-Side Logging**: Add more detailed logging on the server side to help diagnose WebSocket connection issues.

## Implementation Plan

1. **Update `AgentClientOptions`**:
   Enhance the pathPattern option to provide more flexibility:

   ```typescript
   export interface AgentClientOptions {
     // ... existing options
     /** 
      * Path pattern for WebSocket endpoint
      * Supports variables:
      * - $AGENT: replaced with agent name
      * - $INSTANCE: replaced with instance name 
      * @default "/api/agent/$AGENT/$INSTANCE"
      */
     pathPattern?: string;
   }
   ```

2. **Add Path Variation Testing**:
   Modify the connection logic to try different path patterns if the initial connection fails:

   ```typescript
   const pathPatterns = [
     this.options.pathPattern || 'api/agent/$AGENT/$INSTANCE',
     'api/agents/$AGENT/$INSTANCE',
     'ws/agent/$AGENT/$INSTANCE',
     '$AGENT/$INSTANCE'
   ];
   ```

3. **Fix Connection Status Reporting**:
   Only report "connected" when the WebSocket is actually in the OPEN state, not just when the client object is created:

   ```typescript
   // In useChat.ts:
   if (client) {
     setAgentConnection({
       isConnected: false, // Start as false
       client,
       utils
     });
     
     // Create a separate effect to track actual connection status
     useEffect(() => {
       // Function to check WebSocket status
       const checkConnectionStatus = () => {
         const socket = (client as AgentClient).socket;
         const isConnected = socket && socket.readyState === WebSocket.OPEN;
         
         setAgentConnection(prev => ({
           ...prev,
           isConnected
         }));
       };
       
       // Check status immediately
       checkConnectionStatus();
       
       // Set up event listeners for connection status changes
       const handleConnect = () => checkConnectionStatus();
       const handleDisconnect = () => checkConnectionStatus();
       
       client.on('connect', handleConnect);
       client.on('disconnect', handleDisconnect);
       
       // Check status periodically as a fallback
       const interval = setInterval(checkConnectionStatus, 2000);
       
       return () => {
         client.off('connect', handleConnect);
         client.off('disconnect', handleDisconnect);
         clearInterval(interval);
       };
     }, [client]);
   }
   ```

## Root Cause

The 404 errors indicate that the server is not handling WebSocket upgrade requests at the path we're using. Cloudflare Workers hosting Durable Objects have specific routing requirements for WebSocket connections. 

From the Cloudflare documentation, WebSocket connections to Durable Objects need to:
1. Target the correct subdomain/route
2. Include the correct path to the Durable Object
3. Use the correct credentials/authentication

The URL pattern needs to match what the server is expecting.

## Related Files

1. `/packages/core/src/mcp/agent-sdk-bridge.ts` - WebSocket client implementation
2. `/packages/core/src/chat/agent-connection.ts` - Agent connection utilities
3. `/packages/core/src/chat/useChat.ts` - React hook for agent connection
4. `/packages/agents/src/server.ts` - Server implementation (need to check this file)

## Next Steps

1. Examine the server-side code in the Agents package to determine the correct WebSocket endpoint path
2. Test different path patterns to find the one that works
3. Update the client to use the correct path pattern
4. Fix the connection status reporting to accurately reflect the WebSocket connection state

With these changes, we can resolve the 404 errors and establish a proper WebSocket connection to the agent.
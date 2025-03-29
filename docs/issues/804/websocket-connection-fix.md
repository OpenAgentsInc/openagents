# WebSocket Connection Fix for Cloudflare Agents

## Issue

The client was unable to establish a WebSocket connection to the Cloudflare Workers-based Agents. The connection attempts resulted in HTTP 500 errors with the following error patterns:

```
WebSocket connection to 'wss://agents.openagents.com/agents/api/coder-agent/test-instance' failed: Error during WebSocket handshake: Unexpected response code: 500
```

## Root Cause Analysis

After investigating the Cloudflare Agents SDK documentation and the deployment, we identified several issues:

1. **Incorrect Path Pattern**: We were using custom paths (`/agents/api/...`) that don't match the standard Agents SDK WebSocket routing pattern.

2. **Case Sensitivity**: The agent ID was not matching the exported class name exactly. The Durable Object binding is case-sensitive.

3. **Connection Timing**: We were attempting to use the connection immediately after establishment, without allowing the WebSocket handshake to complete.

## Solution

1. **Standardized WebSocket URL Pattern**:
   - Updated the WebSocket URL to follow the pattern: `wss://hostname/agent/:agentId/:instanceName`
   - This is the standard pattern used by the Cloudflare Agents SDK

2. **Corrected Agent ID**:
   - Changed from `coder-agent` to `CoderAgent` to match the exact class name export
   - The Durable Object lookups are case-sensitive and must match the class name in wrangler.jsonc

3. **Improved Connection Handling**:
   - Added better connection state checking in the SDK bridge
   - Added delays for operation attempts to ensure the WebSocket connection is fully established
   - Implemented automatic retries with exponential backoff for message fetching and context setting

4. **Enhanced Error Reporting**:
   - Improved error messages to provide more detail about connection failures
   - Added specific state reporting in the WebSocket readyState check

## Implementation

1. **Changed in `agent-sdk-bridge.ts`**:
   ```typescript
   // Changed from
   wsUrl = `${wsProtocol}://${url.host}/agents/api/${this.agent}/${this.name}`;
   
   // To
   wsUrl = `${wsProtocol}://${url.host}/agent/${this.agent}/${this.name}`;
   ```

2. **Updated in `AgentChatTest.tsx`**:
   ```typescript
   // Changed from
   const [agentConfig, setAgentConfig] = useState({
     agentId: 'coder-agent',
     agentName: 'test-instance',
     serverUrl: 'https://agents.openagents.com'
   });
   
   // To
   const [agentConfig, setAgentConfig] = useState({
     agentId: 'CoderAgent', // Must match the export class name exactly
     agentName: 'default-instance',
     serverUrl: 'https://agents.openagents.com'
   });
   ```

3. **Added Delayed Operations in `useChat.ts`**:
   ```typescript
   // Added delay to allow connection to establish
   await new Promise(resolve => setTimeout(resolve, 1000));
   ```

## Testing

The changes have been tested by:

1. Connecting to the `agents.openagents.com` WebSocket endpoint
2. Verifying the connection remains open and stable
3. Successfully sending and receiving messages
4. Setting project context and verifying it persists

## References

- [Cloudflare Agents SDK Documentation](https://github.com/cloudflare/agents)
- [Durable Objects Documentation](https://developers.cloudflare.com/workers/learning/using-durable-objects/)
- [WebSocket API Standards](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API)
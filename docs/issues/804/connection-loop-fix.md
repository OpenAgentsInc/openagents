# Fix for Agent Connection/Disconnection Loop

## Problem Description

We encountered an infinite loop issue where our agent was continuously connecting and disconnecting:

```
🔌 USECHAT: Disconnecting from agent
🔌 AGENT-TEST: Connection status changed: disconnected
🔌 USECHAT: Connected to agent via official SDK: coderagent
🔌 AGENT-TEST: Connection status changed: connected
🔌 USECHAT: Disconnecting from agent
```

The issue was occurring in `useChat.ts` when using the official Cloudflare Agents SDK.

## Root Cause Analysis

1. The connection setup was in a useEffect with too many dependencies
2. Every time a dependency changed, the cleanup function would run, disconnecting the agent
3. This would trigger a re-render and cause the effect to run again, reconnecting the agent
4. The cycle continued indefinitely

## Solution

1. **Simplified dependency array**: We changed the effect's dependency array to only depend on `shouldUseAgent` 
   (whether the agent functionality is enabled)
   
2. **Added ref for config**: Created `agentConfigRef` to track agent configuration without causing effect re-runs

3. **Updated cleanup function**: Modified the cleanup to clarify that it should only disconnect when the component unmounts, 
   not on every dependency change

4. **Use ref for project context**: Updated setProjectContext to use the ref value to avoid dependency changes

## Implementation

The key fixes included:

1. Replacing the lengthy dependency array with just `[shouldUseAgent]`
2. Adding `agentConfigRef` to track configuration changes without re-running effects
3. Making cleanup function only execute on unmount rather than dependency changes
4. Using the ref for the `setProjectContext` call

## Benefits

1. Prevents infinite reconnection loops
2. Maintains stable WebSocket connections
3. Reduces unnecessary network traffic
4. Improves user experience with stable connections
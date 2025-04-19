# Agent Client Integration Guide

This document provides the correct approach for integrating with OpenAgents in client code, particularly focusing on token handling.

## RPC Method Limitations

There are specific limitations when working with Cloudflare Durable Objects and the Cloudflare Agents SDK:

1. **Async Methods Cannot Be Called Over RPC**: Methods like `setGithubToken` that are async in the agent implementation are not directly callable over RPC.

2. **Use Message Passing Instead**: Always use message passing via WebSockets for operations that can't be done via RPC.

## Correct Way to Initialize Hooks

When creating hooks to interact with agents, implement the token setting like this:

```typescript
import { useState, useCallback } from "react";
import { useAgent } from "agents/react";

export function useOpenAgent(id: string, type: "solver" | "coder") {
  const [state, setState] = useState({ messages: [] });
  
  // Connect to agent using Cloudflare Agents SDK
  const cloudflareAgent = useAgent({
    name: `${type}-${id}`,
    agent: type,
    onStateUpdate: (newState) => {
      console.log(`[useOpenAgent ${type}-${id}] State updated from agent:`, newState);
      setState(newState);
    }
  });
  
  // CORRECT implementation of setGithubToken - uses message passing, not RPC
  const setGithubToken = useCallback(async (token: string): Promise<void> => {
    console.log(`[useOpenAgent ${type}-${id}] Setting GitHub token...`);
    
    // DO NOT call 'setGithubToken' directly via RPC
    // Instead, send a message to set the token
    try {
      await cloudflareAgent.sendMessage(JSON.stringify({
        type: 'set_github_token',
        token: token
      }));
      console.log(`[useOpenAgent ${type}-${id}] Token set message sent`);
    } catch (error) {
      console.error(`[useOpenAgent ${type}-${id}] Failed to set GitHub token:`, error);
      throw error;
    }
    return;
  }, [cloudflareAgent, type, id]);
  
  // Other methods like sendMessage, handleSubmit, etc.
  
  return {
    state,
    messages: state.messages || [],
    setGithubToken,
    // ...other methods
  };
}
```

## Component Usage

When using the hook in a component:

```typescript
function SolverComponent({ issueId, token }) {
  const agent = useOpenAgent(issueId, "solver");
  
  const connectToAgent = async () => {
    try {
      // CORRECT: Use the hook's setGithubToken method which sends a message
      await agent.setGithubToken(token);
      
      // Continue with other setup...
    } catch (error) {
      console.error("Error connecting to agent:", error);
    }
  };
  
  return (
    // Component JSX
  );
}
```

## Server-Side Handling

On the server, the agent needs to handle the token message:

```typescript
// Inside the agent's onMessage handler
case "set_github_token":
  console.log("Setting GitHub token via dedicated message");
  
  try {
    const token = parsedMessage.token;
    if (token && typeof token === 'string') {
      console.log(`Setting GitHub token from message (length: ${token.length})`);
      
      // Don't call this.setGithubToken directly - use updateState instead
      this.updateState({
        githubToken: token
      });
      
      console.log("✓ GitHub token applied to state directly");
      
      // Send success response back to client
      connection.send(JSON.stringify({
        type: "token_response",
        success: true,
        message: "GitHub token set successfully",
        timestamp: new Date().toISOString()
      }));
    } else {
      // Error handling...
    }
  } catch (error) {
    // Error handling...
  }
  break;
```

## Alternative Token Passing Methods

In addition to the message-based approach, you can use these methods (but they are less reliable):

1. **Inference Parameters**:
   ```typescript
   await agent.sendRawMessage({
     type: "shared_infer",
     params: {
       // Other params...
       githubToken: token
     }
   });
   ```

2. **Tool Parameters**:
   ```typescript
   const result = await fetchFileContents({
     owner: "openagentsinc",
     repo: "openagents",
     path: "README.md",
     token: token // Explicit token parameter
   });
   ```

## Troubleshooting

If you encounter this error:
```
RPC error: Error: Method setGithubToken is not callable
```

It means you're trying to use RPC to call the method directly. Check your code:

1. Are you using `agent.call('setGithubToken', [token])`? Change to message passing.
2. Are you using `await agent.setGithubToken(token)`? Change to message passing.

Instead, use:
```typescript
await agent.sendMessage(JSON.stringify({
  type: 'set_github_token',
  token: token
}));
```

Or if your agent hook has a proper implementation:
```typescript
await agent.setGithubToken(token); // Which internally uses message passing
```
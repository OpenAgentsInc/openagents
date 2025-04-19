# GitHub Token Handling Guide

This document explains how to properly handle GitHub tokens with agents in OpenAgents.

## Critical Warning

**NEVER CALL `agent.setGithubToken()` DIRECTLY FROM CLIENT CODE!** 

This method is not callable over RPC and will result in the error:

```
RPC error: Error: Method setGithubToken is not callable
```

This applies to any usage of `cloudflareAgent.call('setGithubToken', [token])` or similar RPC-style calls!

## Correct Token Handling

Always use the message-based approach to set the GitHub token:

### 1. From Client to Agent

```typescript
// CORRECT way to set GitHub token
await agent.sendRawMessage({
  type: "set_github_token",
  token: githubToken
});
```

### 2. In the Solver Component

```typescript
// In your component
const connectToSolver = async () => {
  setConnectionState('connecting');

  // Set GitHub token via message
  await agent.sendRawMessage({
    type: "set_github_token",
    token: githubToken
  });
  
  // Continue with other setup...
};
```

### 3. In the `useOpenAgent` Hook

```typescript
// CORRECT implementation in the hook
const setGithubToken = async (token: string): Promise<void> => {
  // DO NOT call 'setGithubToken' via RPC
  await cloudflareAgent.sendMessage(JSON.stringify({
    type: 'set_github_token',
    token: token
  }));
  return;
}
```

## Additional Token Mechanisms

The Solver agent also supports these additional token handling mechanisms:

### 1. Passing Token in Inference Parameters

You can pass the token in the inference parameters, which will set it in the state for that operation:

```typescript
await agent.sendRawMessage({
  type: "shared_infer",
  params: {
    // Other params...
    githubToken: token
  }
});
```

This method sets the token only for the current inference operation but doesn't permanently store it for future operations.

### 2. Direct GitHub Tool Parameters

For GitHub operations, you can provide the token directly in the tool parameters:

```typescript
// When using a GitHub tool directly
const result = await fetchFileContents({
  owner: "openagentsinc",
  repo: "openagents",
  path: "README.md",
  token: githubToken // Explicit token parameter
});
```

## How Token Handling Works Internally

1. When the agent receives a `set_github_token` message, it calls its internal `setGithubToken` method to store the token in the agent's state.

2. For inference operations with a token parameter, the agent temporarily sets the token for that operation.

3. When GitHub tools are executed, they first check for an explicit token parameter, then fall back to the token in the agent's state.

## Troubleshooting

If you encounter GitHub token errors:

1. Ensure you're sending the token via the `set_github_token` message type, not trying to call the method directly.

2. Check that the token is valid and has the required scopes.

3. Verify that the token is being properly set by inspecting the agent's logs.

4. Use the `verify_token` command to check if the token is properly set:
   ```typescript
   await agent.sendRawMessage({
     type: "command",
     command: "verify_token"
   });
   ```
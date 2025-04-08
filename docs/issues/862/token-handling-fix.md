# GitHub Token Handling Fix in Cloudflare Workers

## Issue

When deploying the agent server to Cloudflare Workers, we encountered the following error:

```
Error parsing body: Error: asyncLocalStorage.enterWith() is not implemented
```

This occurred because the codebase was using Node.js's `AsyncLocalStorage` API to pass the GitHub token through the execution context, but this API is not supported in the Cloudflare Workers runtime environment.

## Root Cause

The original implementation in `packages/agents/src/server.ts` was using the `AsyncLocalStorage` API to store and access the GitHub token across asynchronous contexts:

```typescript
import { AsyncLocalStorage } from "node:async_hooks";

export const toolContext = new AsyncLocalStorage<Coder>();

export class Coder extends AIChatAgent<Env> {
  protected githubToken?: string;
  
  extractToken(connection: Connection, message: WSMessage) {
    // ... token extraction logic ...
    
    // Store in the context for tools to access
    toolContext.enterWith(this);
  }
}
```

This approach fails in Cloudflare Workers because:

1. The `node:async_hooks` module is not available in Cloudflare Workers
2. The `AsyncLocalStorage` API is specific to Node.js
3. Cloudflare Workers uses a different runtime model than Node.js

## Solution

We implemented a simpler approach that doesn't rely on AsyncLocalStorage:

1. Made the `githubToken` property public in the `Coder` class
2. Added a `getGitHubToken()` method to access the token
3. Updated tools to access the token directly from the agent instance
4. Modified the GitHub plugin to get the token from the agent instance

### Key Changes

#### 1. Updated the Coder class in server.ts:

```typescript
export class Coder extends AIChatAgent<Env> {
  // Make it public so tools can access it
  public githubToken?: string;
  
  extractToken(connection: Connection, message: WSMessage) {
    // ... token extraction logic ...
    
    // Just set the token directly on the instance
    this.githubToken = githubToken;
    // No AsyncLocalStorage usage
  }
  
  // Public method to get the GitHub token for tools
  getGitHubToken(): string | undefined {
    return this.githubToken;
  }
}
```

#### 2. Updated tools to access the agent instance directly:

```typescript
const myTool = tool({
  // ...
  execute: async (args, { agent }) => {
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
    }
    
    // Access token directly from the agent instance
    const token = agent.githubToken;
    
    // ...
  }
});
```

#### 3. Updated the GitHub plugin to access the token from the agent:

```typescript
class OpenAIAgentPlugin implements AgentPlugin {
  // ...
  
  async initialize(agent: AIChatAgent<any>): Promise<void> {
    this.agent = agent;
    
    // Try to get token from agent properties
    if ((agent as any).githubToken) {
      this.githubToken = (agent as any).githubToken;
    }
    // ...
  }
  
  private getGitHubToken(): string | undefined {
    // 1. Check if we have a token stored in the instance
    if (this.githubToken) {
      return this.githubToken;
    }
    
    // 2. Check if the agent has a token
    if (this.agent && (this.agent as any).githubToken) {
      return (this.agent as any).githubToken;
    }
    
    // ...
  }
}
```

## Why This Works

1. The Cloudflare Workers environment passes the agent instance to tool execution callbacks
2. By making the token accessible directly on the agent instance, we avoid the need for AsyncLocalStorage
3. This approach follows a more explicit parameter-passing pattern than relying on implicit context

## Benefits of the New Approach

1. **Simplicity**: No reliance on Node.js-specific APIs
2. **Compatibility**: Works across different runtimes including Cloudflare Workers
3. **Transparency**: More explicit flow of the token through the system
4. **Maintainability**: Easier to understand and debug

## Testing

The fix was verified by:

1. Confirming the token is correctly extracted from the incoming WebSocket messages
2. Verifying the token is accessible in the tools via the agent instance
3. Testing GitHub API operations that require authentication

## Conclusion

By removing the dependency on Node.js-specific AsyncLocalStorage and adopting a more direct approach to token handling, we've made the agent server compatible with the Cloudflare Workers environment while maintaining the same functionality.
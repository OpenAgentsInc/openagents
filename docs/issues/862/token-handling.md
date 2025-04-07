# GitHub Token Handling Implementation (Issue #862)

## Overview

This document explains the implementation of GitHub token handling in the OpenAgents system, specifically focusing on extracting tokens from WebSocket messages and making them available to MCP (Model Context Protocol) tools.

## Problem Statement

GitHub tokens were not reliably passed from the user interface to the MCP tools, causing GitHub operations to fail with 522 timeout errors, even after tokens were configured in the API Keys settings page.

## Root Cause Analysis

After extensive debugging and logging, we discovered:

1. **Transport Method**: The GitHub token was being passed in the body of WebSocket messages, not as HTTP headers.
2. **Message Structure**: The token was embedded within the `cf_agent_use_chat_request` message type sent over WebSocket.
3. **Missing Extraction Point**: Previous implementations attempted to extract the token from HTTP headers or request body, but tokens were actually in WebSocket message data.
4. **Token Access Timing**: The agent needed access to the token during tool execution.

## Implementation Solution

### 1. WebSocket Message Handling Architecture

The token flow now follows this path:

```
UI Settings → WebSocket Message Body → Agent Environment → AsyncLocalStorage Context → MCP Tools
```

### 2. UI Components (ChatPage.tsx)

The token is retrieved from the API Keys settings and passed in the request data:

```typescript
// --- API Keys ---
const { apiKeys } = useApiKeyContext();

// --- Initialize Agent ---
const agent = useAgent({
  agent: "coder",
  headers: {
    "x-api-key": apiKeys.github || "",
    "x-github-token": apiKeys.github || "",
  },
});

// --- Agent Chat Hook ---
const { /* ... */ } = useAgentChat({
  data: {
    apiKeys,
  },
  agent,
  // ...
});
```

### 3. WebSocket Message Parsing (server.ts)

The core of the solution is overriding the `onMessage` method in the Coder agent class to extract the token directly from WebSocket messages:

```typescript
// Create a context for tools to access the GitHub token
const toolContext = new AsyncLocalStorage<{
  githubToken?: string;
  tools: Record<string, any>;
}>();

export class Coder extends AIChatAgent<Env> {
  /**
   * Override onMessage to extract GitHub token from request body
   */
  override async onMessage(connection: Connection, message: WSMessage) {
    if (typeof message === "string") {
      let data: IncomingMessage;
      try {
        data = JSON.parse(message) as IncomingMessage;
        
        if (data.type === "cf_agent_use_chat_request" && data.init.method === "POST") {
          // Parse the request body
          const { body } = data.init;
          if (body) {
            const requestData = JSON.parse(body as string);
            
            // Extract token from different possible locations
            const githubToken = 
              requestData.githubToken || 
              (requestData.data?.apiKeys?.github) || 
              (requestData.apiKeys?.github);
            
            if (githubToken) {
              // Set token in environment for the agent
              const env = (this as any).env as Env;
              if (env) {
                if (!env.apiKeys) env.apiKeys = {};
                env.apiKeys.github = githubToken;
                env.GITHUB_TOKEN = githubToken;
                env.GITHUB_PERSONAL_ACCESS_TOKEN = githubToken;
              }
              
              // Set up tool context with GitHub token
              const context = {
                githubToken,
                tools,
              };
              
              // Run the rest of the message handling with the tool context
              return toolContext.run(context, async () => {
                return super.onMessage(connection, message);
              });
            }
          }
        }
      } catch (error) {
        console.error("Error parsing message JSON:", error);
      }
    }
    
    return super.onMessage(connection, message);
  }
}
```

### 4. Tool Execution Context

When handling chat messages, we check for and utilize the token from the AsyncLocalStorage context:

```typescript
async onChatMessage(onFinish: StreamTextOnFinishCallback<any>) {
  // Get context with GitHub token if available
  const context = toolContext.getStore();
  if (context?.githubToken) {
    console.log(`Using GitHub token from context (length: ${context.githubToken.length})`);
  }
  
  // Create a streaming response with the agent context
  return agentContext.run(this, async () => {
    // Process messages and execute tools with GitHub token in context
    // ...
  });
}
```

### 5. GitHub Plugin Token Access (github-plugin.ts)

The GitHub plugin's `getGitHubToken` method has been simplified with clear token precedence:

```typescript
private getGitHubToken(paramToken?: string): string | undefined {
  // 1. Prioritize token passed directly to the tool method
  if (paramToken && paramToken.trim() !== '') {
    return paramToken;
  }
  
  // 2. Attempt to use the token from the agent environment
  const env = this.getAgentEnv();
  if (!env) return undefined;
  
  // Check in standard locations, in order of precedence
  if (env.GITHUB_TOKEN && typeof env.GITHUB_TOKEN === 'string') {
    return env.GITHUB_TOKEN;
  }
  
  if (env.GITHUB_PERSONAL_ACCESS_TOKEN && typeof env.GITHUB_PERSONAL_ACCESS_TOKEN === 'string') {
    return env.GITHUB_PERSONAL_ACCESS_TOKEN;
  }
  
  // Check apiKeys as final option
  if (env.apiKeys && env.apiKeys.github && typeof env.apiKeys.github === 'string') {
    return env.apiKeys.github;
  }
  
  // No token found
  return undefined;
}
```

## Key Improvements

1. **Direct Message Extraction**: Extract token directly from WebSocket messages where it's actually being passed.
2. **AsyncLocalStorage Context**: Use Node.js AsyncLocalStorage to make the token accessible during tool execution.
3. **Enhanced Logging**: Detailed logging at each step for better debugging and transparency.
4. **Removed Placeholder Tokens**: Eliminated debug tokens to ensure real failures are visible ("fail fast" approach).
5. **Multiple Path Support**: Handle tokens passed through different paths in the message body.

## Error Handling

Improved error messages guide users to add tokens in the API Keys settings:

```typescript
if (!tokenToSend) {
  console.warn(`⚠️ No GitHub token is being sent for MCP tool ${toolName}.`);
  console.warn(`⚠️ This operation may fail if accessing private repositories.`);
  console.warn(`Please add a GitHub token in Settings > API Keys to enable full GitHub functionality.`);
}
```

For authentication failures:
```typescript
if (status === 401 || status === 403) {
  return JSON.stringify({
    error: `Authentication error (${status}) from GitHub. A valid token with correct permissions is required. Please add a GitHub token in Settings > API Keys.`
  });
}
```

## Testing

To test the GitHub token handling:

1. Configure a token in the API Keys settings page
2. Connect to the agent via WebSocket
3. Send a message that uses a GitHub tool
4. Check logs for:
   - Message parsing and token extraction
   - Token presence in AsyncLocalStorage context
   - Successful GitHub API operation

## Debugging

If GitHub operations still fail, the detailed logs should identify the issue location:

1. `onMessage` parsing logs will show if the token extraction is working
2. `onChatMessage` logs will show if the token is being accessed from the context
3. GitHub plugin logs will show which token source is being used
4. MCP tool execution logs will show if the token is being passed to API calls

This approach directly addresses the core issue by focusing on the actual message transport mechanism rather than incorrect assumptions about where the token is passed.
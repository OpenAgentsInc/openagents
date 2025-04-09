# GitHub Token Handling Fix (Issue #862)

## Issue Summary

The issue concerned inconsistencies in GitHub token handling across the application, particularly:

1. The GitHub API MCP server expected a `token` parameter in requests to authenticate operations
2. This token wasn't being passed correctly from the Coder agent to the MCP tools
3. As a result, operations requiring authentication (like commenting, creating issues, etc.) would fail while read-only operations worked

## Implementation

The fix focuses on explicitly passing the GitHub token to the MCP tools by:

### 1. Token Capture in the Coder Agent

The Coder class already had a `githubToken` property set from WebSocket messages:

```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  // Check if this is a chat request with a GitHub token
  if (typeof message === 'object' && message !== null) {
    const chatMessage = message as any;
    if (chatMessage.type === "cf_agent_use_chat_request" && chatMessage.data) {
      const data = chatMessage.data;
      if (data.githubToken) {
        await this.updateGitHubToken(data.githubToken);
      }
    }
  }
}
```

### 2. Token Inclusion in Tool Arguments

In the `refreshServerData` method, we modified the tool wrappers to include the token in arguments:

```typescript
// Get the token from the context for use in the closure
const coderAgent = this;

// Create a tool wrapper
const toolWrapper = tool({
  description: typedToolDef.description,
  parameters: typedToolDef.parameters,
  execute: async (args, options) => {
    try {
      // Include the GitHub token in the arguments if available
      const githubToken = coderAgent.githubToken;
      const argsWithToken = githubToken ? { ...args, token: githubToken } : args;
      
      console.log(`Executing MCP tool ${name} with args:`, JSON.stringify(args));
      console.log(`Token presence for ${name}: ${!!githubToken}`);
      
      // Pass arguments with token to the MCP tool
      return await typedToolDef.execute(argsWithToken, options);
    } catch (error: unknown) {
      // Error handling...
    }
  }
});
```

### 3. MCP Server Token Processing

The MCP GitHub server already had the code to use the token in the request:

```typescript
this.server.tool(tool.name, tool.schema.shape, async (params: Record<string, unknown>) => {
  const validatedParams = tool.schema.parse(params);
  const context: ToolContext = {
    token: (params as { token?: string }).token,
  };

  // Temporarily replace githubRequest with token-aware version
  const originalRequest = globalThis.githubRequest;
  try {
    console.log(`🔧 Executing GitHub tool: ${tool.name}`);
    console.log(`📊 Tool parameters: ${JSON.stringify(validatedParams).substring(0, 200)}`);
    console.log(`🔑 GitHub token present: ${!!context.token}`);
    
    globalThis.githubRequest = withToken(context.token);
    const result = await tool.handler(validatedParams as any);
    
    // Rest of the code...
```

### 4. Improved Error Handling

We enhanced the error handling to provide better feedback about token-related issues:

```typescript
return {
  error: `The GitHub tool "${name}" failed: ${errorMessage}`,
  toolName: name,
  // Include original args for context (without token for security)
  args: args,
  // Add a note about token if missing for write operations
  tokenInfo: coderAgent.githubToken ? 
    "Token was provided but still encountered an error" : 
    "No GitHub token was provided. Some operations require authentication."
};
```

## Key Improvements

1. **Token Propagation**: The GitHub token is now properly passed through the entire execution chain
2. **Scope Management**: We use a closure to maintain access to the Coder agent instance and its token
3. **Transparent Error Handling**: Error messages now clearly indicate whether token-related issues occurred
4. **Security**: We avoid logging the full token while still providing debugging information

## Validation

The fix was validated by successfully using writing operations (like commenting on PRs) that previously failed. These operations now work correctly when a valid GitHub token is provided.

## Further Considerations

1. **Token Refresh**: Currently we don't handle token expiration or refreshing
2. **Token Scopes**: Different operations may require different token scopes
3. **Error Feedback**: The AI model could suggest creating a new token if specific permissions are needed

## Conclusion

This fix ensures that when a GitHub token is provided by the user, it's properly passed through all layers of the application to the GitHub API calls. This resolves the authentication issues for operations that require write access or access to private repositories.
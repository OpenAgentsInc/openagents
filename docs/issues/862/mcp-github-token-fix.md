# MCP GitHub Token Fix (Issue #862)

## Issue Summary

The GitHub token handling was not working correctly, preventing write operations like adding comments or creating issues. The primary issues were:

1. The agent was not extracting the token from incoming messages correctly
2. The token was not being passed to MCP tools properly
3. Connection issues with the MCP server were occurring, compounding the issue

## Implementation Approach

After analyzing the issue, we implemented a focused solution:

### 1. Message Handling

The problem started with message parsing. The token is sent from the client in the `body.githubToken` property:

```typescript
// In ChatPage.tsx:
useAgentChat({
  body: {
    githubToken: apiKeys['github'] || ''
  },
  agent,
  // ...
});
```

But our server code wasn't correctly extracting this token. We've fixed this by updating the `onMessage` method:

```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  // Call the parent method first
  await super.onMessage(connection, message);
  
  // Cast message to any to avoid TypeScript errors with property access
  const msg = message as any;
  
  // Only check for githubToken in object messages
  if (typeof msg === 'object' && msg !== null) {
    try {
      if (msg.data && msg.data.githubToken) {
        console.log("Found githubToken in message.data");
        this.githubToken = msg.data.githubToken;
      } else if (msg.body && msg.body.githubToken) {
        console.log("Found githubToken in message.body");
        this.githubToken = msg.body.githubToken;
      }
    } catch (e) {
      console.error("Error extracting token:", e);
    }
  }
}
```

### 2. Token Transfer to Tools

Once extracted, the token needs to be passed to each tool invocation. We've implemented this in the tool creation process:

```typescript
// Create a tool wrapper with the token
const coderAgent = this; // Capture agent context for closure

const toolWrapper = tool({
  description: typedToolDef.description,
  parameters: typedToolDef.parameters,
  execute: async (args, options) => {
    try {
      // Get fresh token value at execution time
      const githubToken = coderAgent.githubToken;
      
      // Add GitHub token to arguments
      const argsWithToken = githubToken ? { ...args, token: githubToken } : args;
      
      // Simple logging for debugging
      console.log(`Executing MCP tool ${name} with token: ${githubToken ? "YES" : "NO"}`);
      
      // Execute the tool with token
      return await typedToolDef.execute(argsWithToken, options);
    } catch (error: unknown) {
      // Error handling...
    }
  }
});
```

### 3. MCP Client Management

We also improved the MCP client handling to ensure connection reliability:

```typescript
async refreshServerData() {
  // Initialize MCP client if needed
  if (!this.mcpClient) {
    this.mcpClient = await createMCPClient({
      transport: {
        type: 'sse' as const,
        url: "https://mcp-github.openagents.com/sse"
      },
      name: "coder"
    });
  }

  // Test the connection
  try {
    const testTools = await this.mcpClient.tools();
  } catch (error) {
    // Reinitialize on failure
    this.mcpClient = await createMCPClient({
      transport: {
        type: 'sse' as const,
        url: "https://mcp-github.openagents.com/sse"
      },
      name: "coder"
    });
  }
  
  // Process tools...
}
```

## MCP Server Behavior

The MCP GitHub server (in `apps/mcp-github-server/src/index.ts`) expects the token in the tool parameters:

```typescript
this.server.tool(tool.name, tool.schema.shape, async (params: Record<string, unknown>) => {
  const validatedParams = tool.schema.parse(params);
  const context: ToolContext = {
    token: (params as { token?: string }).token,
  };

  // Temporarily replace githubRequest with token-aware version
  globalThis.githubRequest = withToken(context.token);
  const result = await tool.handler(validatedParams as any);
  
  // Rest of the code...
});
```

The key line is `token: (params as { token?: string }).token`, which extracts the token from the params object.

## Testing

When the token is correctly passed:

1. The log will show `Executing MCP tool <name> with token: YES`
2. Write operations like `add_issue_comment` will succeed
3. No "Requires authentication" errors will be shown

## Remaining Considerations

1. **Error Handling**: Clear error messages are returned when authentication fails
2. **Connection Management**: The MCP client connection is tested and reinitialized if needed
3. **Performance**: Token is passed efficiently without unnecessary operations

This implementation ensures that GitHub tokens are correctly extracted from messages and passed to the appropriate MCP tools for authentication.
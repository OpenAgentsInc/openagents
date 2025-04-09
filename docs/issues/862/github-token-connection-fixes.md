# GitHub Token and Connection Fixes (Issue #862)

## Issues Identified

Based on debugging logs, three critical issues were identified:

1. **No GitHub Token Available**: The token wasn't properly extracted from messages or persisted through the application flow
2. **MCP Connection Errors**: The SSE connection to the MCP server was failing with "Not connected" errors
3. **Message Format Incompatibility**: The WebSocket messages were coming in as strings instead of objects, preventing proper token extraction

## Implemented Fixes

### 1. Robust Message Processing

Enhanced the `onMessage` method to handle various message formats:

```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  // Try to parse string messages as JSON
  let parsedMessage: any = message;
  if (typeof message === 'string') {
    try {
      parsedMessage = JSON.parse(message);
      console.log(`PARSED_MESSAGE: Successfully parsed message from string to object`);
    } catch (e) {
      console.log(`PARSE_ERROR: Failed to parse message as JSON: ${e}`);
    }
  }
  
  // Try different message structures to find the token
  if (parsedMessage.type === "cf_agent_use_chat_request" && parsedMessage.data) {
    await this.processMessageData(parsedMessage.data);
  } else if (parsedMessage.data && typeof parsedMessage.data === 'object') {
    await this.processMessageData(parsedMessage.data);
  } else if (parsedMessage.githubToken || parsedMessage.apiKeys) {
    await this.processMessageData(parsedMessage);
  } else {
    // Deep search in all message properties
    for (const key of Object.keys(parsedMessage)) {
      if (typeof parsedMessage[key] === 'object' && parsedMessage[key] !== null) {
        await this.processMessageData(parsedMessage[key]);
      }
    }
  }
  
  // Fallback to environment variables if no token was found
  if (!this.githubToken && env.GITHUB_TOKEN) {
    await this.updateGitHubToken(env.GITHUB_TOKEN);
  }
}
```

### 2. MCP Connection Management

Added robust connection management to ensure the MCP client remains connected:

```typescript
async refreshServerData() {
  // Initialize MCP client if needed
  if (!this.mcpClient) {
    try {
      this.mcpClient = await createMCPClient({
        transport: {
          type: 'sse' as const,
          url: "https://mcp-github.openagents.com/sse"
        },
        name: "coder"
      });
    } catch (error) {
      console.error("Failed to initialize MCP client:", error);
      return;
    }
  }

  // Test the connection before proceeding
  try {
    const testTools = await this.mcpClient.tools();
    console.log(`MCP client connection verified, found ${Object.keys(testTools).length} tools`);
  } catch (error) {
    // Reinitialize if test fails
    this.mcpClient = await createMCPClient({
      transport: {
        type: 'sse' as const,
        url: "https://mcp-github.openagents.com/sse"
      },
      name: "coder"
    });
  }
  
  // Continue with tool retrieval...
}
```

### 3. Resilient Tool Execution

Enhanced tool execution with connection checks and automatic retries:

```typescript
execute: async (args, options) => {
  try {
    // Get token and add to arguments
    const finalToken = coderAgent.githubToken;
    const argsWithToken = finalToken ? { ...args, token: finalToken } : args;
    
    // Verify MCP connection before execution
    if (!coderAgent.mcpClient) {
      await coderAgent.refreshServerData();
    }
    
    // Execute the tool
    return await typedToolDef.execute(argsWithToken, options);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    
    // Automatic retry for connection errors
    if (errorMessage.includes("Not connected") || errorMessage.includes("SSE Transport Error")) {
      try {
        await coderAgent.refreshServerData();
        const githubToken = coderAgent.githubToken;
        const argsWithToken = githubToken ? { ...args, token: githubToken } : args;
        return await typedToolDef.execute(argsWithToken, options);
      } catch (retryError) {
        console.error(`RETRY_FAILED: Failed to retry operation: ${retryError}`);
      }
    }
    
    // Special handling for authentication errors
    if (errorMessage.includes("authentication") || errorMessage.includes("Unauthorized")) {
      return {
        error: `GitHub authentication failed. Please provide a valid GitHub token with sufficient permissions.`,
        authenticationRequired: true
      };
    }
    
    // General error handling
    return {
      error: `The GitHub tool "${name}" failed: ${errorMessage}`,
      tokenInfo: coderAgent.githubToken ? 
        "Token was provided but still encountered an error." : 
        "No GitHub token was provided. Some operations require authentication."
    };
  }
}
```

### 4. Multiple Token Sources

Added support for extracting tokens from different locations:

```typescript
private async processMessageData(data: any) {
  // Check API keys section
  if (data.apiKeys && typeof data.apiKeys === 'object' && data.apiKeys.github) {
    await this.updateGitHubToken(data.apiKeys.github);
    return;
  }
  
  // Check direct token property
  if (data.githubToken) {
    await this.updateGitHubToken(data.githubToken);
    return;
  }
  
  // Check for token property
  if (data.token) {
    await this.updateGitHubToken(data.token);
    return;
  }
}
```

### 5. Environment Variable Fallback

Added fallback to environment variables when no token is provided via messages:

```typescript
// In tool execution
if (!githubToken) {
  const envToken = env.GITHUB_TOKEN;
  if (envToken) {
    console.log(`TOOL_EXEC: Using environment token for ${name}`);
    await coderAgent.updateGitHubToken(envToken);
  }
}
```

## Key Improvements

1. **Resilience**: The system now maintains connection to the MCP server with automatic reconnection
2. **Multiple Token Sources**: Tokens can come from messages, API keys, or environment variables
3. **Message Format Flexibility**: Handles both object and string message formats
4. **Automatic Retries**: Operations retry automatically after connection errors
5. **Improved Error Handling**: Provides clear, actionable error messages based on error type
6. **Deep Diagnostics**: Extensive logging helps diagnose any remaining issues

## Testing the Solution

The solution can be verified by:

1. **Read Operations**: Using `get_file_contents` on a public repository
2. **Write Operations**: Using `add_issue_comment` or `create_or_update_file`
3. **Connection Errors**: Checking logs for connection retries and reconnections
4. **Token Sources**: Testing with tokens from different sources (environment, direct message)

## Conclusion

These enhancements address the core issues with GitHub token handling and MCP connectivity. The implementation is now more robust and resilient, with better error reporting and automatic recovery from common failure modes.
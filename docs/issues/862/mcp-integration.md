# MCP Integration Using Vercel AI SDK

## Overview

This document outlines the implementation plan for replacing our custom Cloudflare MCP (Model Context Protocol) handling with Vercel AI SDK's built-in MCP client. This change will simplify our codebase and provide better maintenance, as we'll leverage Vercel AI SDK's standardized implementation rather than maintaining our own.

## Current Implementation

Currently, our implementation in `packages/agents/src/server.ts` uses a custom `MCPClientManager` class to:

1. Connect to MCP servers (primarily `https://mcp-github.openagents.com/sse`)
2. Retrieve tool definitions from these servers
3. Convert JSON Schema tool definitions to Zod schemas
4. Execute tool calls with appropriate error handling
5. Implement fallback mechanisms (direct GitHub API calls) when MCP fails

Key issues with the current implementation:
- Custom schema conversion logic
- Complex error handling
- Multiple layers of abstraction
- Manual tool management

## Vercel AI SDK MCP Client Features

The Vercel AI SDK provides an `MCPClient` that offers:

1. Built-in SSE transport configuration
2. Automatic conversion of MCP tools to Vercel AI SDK tools
3. Proper error handling
4. Protocol version negotiation
5. Type-safe interfaces for all operations

## Implementation Plan

### 1. Replace MCPClientManager with Vercel AI SDK's MCPClient

```typescript
// Current implementation
this.mcp_ = new MCPClientManager("coder", "0.0.1", {
  baseCallbackUri: `https://agents.openagents.com/agents/coder/${this.name}/callback`,
  storage: this.ctx.storage,
});

// New implementation using Vercel AI SDK
import { createMCPClient } from 'ai/client';
import { createSSETransport } from 'ai/transports/sse';

// Initialize in onStart
async onStart() {
  // Configure SSE transport
  const transport = createSSETransport({
    url: "https://mcp-github.openagents.com/sse",
    credentials: "include",
    // Handle authentication as needed
  });

  // Create MCP client with the transport
  this.mcpClient = await createMCPClient({
    transport,
    clientName: "coder",
    clientVersion: "0.0.1"
  });

  // Initialize built-in tools
  this.combinedTools = { ...builtInTools };
  
  // Fetch and process MCP tools
  await this.refreshServerData();
}
```

### 2. Replace Custom Tool Fetching with SDK's tools() Method

```typescript
async refreshServerData() {
  try {
    // Get MCP tools using the Vercel AI SDK client
    const mcpTools = await this.mcpClient.tools();
    
    console.log("MCP tools retrieved:", Object.keys(mcpTools));
    
    // Filter to only use specific tools if needed
    const filteredTools = {};
    for (const [name, tool] of Object.entries(mcpTools)) {
      if (name === 'get_file_contents') {
        filteredTools[name] = tool;
      }
    }
    
    // Combine with built-in tools
    this.combinedTools = {
      ...builtInTools,
      ...filteredTools
    };
    
    console.log("Combined tools:", Object.keys(this.combinedTools));
  } catch (error) {
    console.error("Failed to refresh MCP tools:", error);
    // Fallback to built-in tools only
    this.combinedTools = { ...builtInTools };
  }
}
```

### 3. Implement GitHub Token Handling

```typescript
// In the message handler or appropriate location:
if (message.data?.type === "cf_agent_use_chat_request") {
  const { githubToken } = message.data;
  if (githubToken) {
    this.githubToken = githubToken;
    
    // Re-initialize MCP client with the token if needed
    if (this.mcpClient) {
      // Configure the transport with the token
      const transport = createSSETransport({
        url: "https://mcp-github.openagents.com/sse",
        credentials: "include",
        headers: {
          "Authorization": `Bearer ${githubToken}`
        }
      });
      
      // Update the client
      this.mcpClient = await createMCPClient({ transport });
      
      // Refresh tools with the new authenticated client
      await this.refreshServerData();
    }
  }
}
```

### 4. Implement Error Handling and Fallbacks

While the Vercel AI SDK handles many errors internally, we should still implement fallbacks for critical operations:

```typescript
// In the tools() execute function:
try {
  // Attempt to execute the MCP tool
  return await tool.execute(args, { signal });
} catch (error) {
  console.error(`MCP tool execution failed: ${error.message}`);
  
  // For get_file_contents, implement GitHub API fallback
  if (toolName === 'get_file_contents') {
    try {
      // Implement GitHub API fallback as in the current code
      const { owner, repo, path, branch } = args;
      // ... [GitHub API implementation] ...
      return result;
    } catch (fallbackError) {
      throw new Error(`Tool execution failed and fallback failed: ${fallbackError.message}`);
    }
  }
  
  // For other tools, just re-throw
  throw error;
}
```

## Benefits of This Approach

1. **Simplified Code**: The Vercel AI SDK handles most of the complexity
2. **Standardized Integration**: Uses well-maintained, documented interfaces
3. **Type Safety**: Better TypeScript integration throughout
4. **Easier Maintenance**: Less custom code to maintain
5. **Future Compatibility**: Easier to upgrade as the MCP protocol evolves

## Implementation Steps

1. Add Vercel AI SDK dependencies if not already present
2. Replace the MCPClientManager initialization with createMCPClient
3. Replace the refreshServerData method to use mcpClient.tools()
4. Implement GitHub token handling
5. Add appropriate error handling and fallbacks
6. Test with the GitHub MCP server (https://mcp-github.openagents.com/sse)
7. Update any other code that depends on the current implementation

## Considerations

- **Authentication**: Ensure the token is properly passed to the MCP server
- **Error Handling**: Preserve the robust error handling from the current implementation
- **Backward Compatibility**: Ensure the API remains compatible for existing code
- **Fallbacks**: Maintain fallback mechanisms for critical operations

## Testing Plan

1. Test connection to the MCP server
2. Verify tool retrieval and conversion
3. Test tool execution with various parameters
4. Test error scenarios (server unavailable, authentication failures)
5. Test GitHub fallback functionality
6. Test with both authenticated and unauthenticated requests

## Conclusion

Migrating to the Vercel AI SDK's MCP client will simplify our codebase, improve maintainability, and ensure we stay compatible with the evolving MCP protocol. The implementation should maintain all current functionality while reducing the amount of custom code we need to maintain.
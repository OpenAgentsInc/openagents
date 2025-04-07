# MCP GitHub Tools Integration with SSE Transport

## Overview

This document explains the implementation details of the Model Context Protocol (MCP) GitHub tools integration using the Server-Sent Events (SSE) transport.

## SSE Transport Implementation

The MCP client uses the SSE transport to communicate with the MCP GitHub server:

```typescript
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";

// Create an SSE transport
const sseUrl = new URL("https://mcp-github.openagents.com/sse");
const transport = new SSEClientTransport(sseUrl);

// Connect to the MCP server with the transport
await this.mcpClient.connect(transport);
```

## Connection Management

The GitHub plugin implements a robust connection management system:

1. **Initialization**: The plugin creates an MCP client during construction
2. **Connection Establishment**: The `ensureConnected()` method establishes a connection when needed
3. **Reconnection Logic**: Automatically reconnects if the connection is lost

```typescript
private async ensureConnected(): Promise<boolean> {
  // If already connected, just return
  if (this.connected && this.mcpClient) {
    return true;
  }
  
  try {
    // Connect to MCP GitHub server
    console.log("Connecting to MCP GitHub server...");
    
    // Recreate the client if needed
    if (!this.mcpClient) {
      this.mcpClient = new Client({
        name: "coder-agent",
        version: "1.0.0"
      });
    }
    
    // Create an SSE transport
    const sseUrl = new URL("https://mcp-github.openagents.com/sse");
    const transport = new SSEClientTransport(sseUrl);
    
    // Connect to the MCP server with the transport
    await this.mcpClient.connect(transport);
    
    console.log("Successfully connected to MCP GitHub server");
    this.connected = true;
    
    // Fetch available tools from MCP server
    console.log("Fetching available MCP tools...");
    const mcpTools = await this.mcpClient.tools();
    
    if (mcpTools) {
      // Store tool definitions for later use
      this.mcpToolsMap = mcpTools;
      console.log(`Fetched ${Object.keys(mcpTools).length} MCP tools`);
    }
    
    return true;
  } catch (error) {
    console.error("Error connecting to MCP server:", error);
    this.connected = false;
    return false;
  }
}
```

## Tool Execution Flow

Each tool call ensures that a connection is established before making the actual request:

```typescript
private async callMCPTool(toolName: string, params: any): Promise<string> {
  // First ensure we're connected to the MCP server
  const isConnected = await this.ensureConnected();
  if (!isConnected || !this.mcpClient) {
    throw new Error("Cannot connect to MCP server");
  }

  try {
    console.log(`Calling MCP tool ${toolName} with params:`, params);
    
    // Add GitHub token if available from environment
    const githubParams = { ...params };
    if (this.agent?.env?.GITHUB_TOKEN) {
      console.log("Using GitHub token from environment");
      githubParams.token = this.agent.env.GITHUB_TOKEN;
    }
    
    // Call the MCP tool
    const result = await this.mcpClient.callTool({
      name: toolName,
      arguments: githubParams
    });
    
    // Process and return the result
    // ...
  } catch (error) {
    // Error handling and reconnection logic
    // ...
  }
}
```

## Error Handling

The implementation includes comprehensive error handling:

1. **Connection Errors**: Automatically attempts to reconnect if a connection-related error occurs
2. **Tool Execution Errors**: Properly formats and handles errors from the MCP server
3. **Authentication Issues**: Supports passing GitHub tokens for authenticated requests

```typescript
// Try to reconnect if the error seems connection-related
if (!this.connected || 
    (error.message && 
     (error.message.includes('connection') || 
      error.message.includes('Not connected') || 
      error.message.includes('transport')))) {
  try {
    console.log("Attempting to reconnect to MCP server after tool error...");
    // Reset the connection flag so ensureConnected will try to reconnect
    this.connected = false;
    const reconnected = await this.ensureConnected();
    
    if (reconnected) {
      console.log("Successfully reconnected to MCP server, retrying tool call");
      // Retry the call
      return this.callMCPTool(toolName, params);
    }
  } catch (reconnectError) {
    console.error("Failed to reconnect to MCP server:", reconnectError);
  }
}
```

## GitHub Token Handling

The plugin supports GitHub authentication tokens from the environment:

```typescript
// Add GitHub token if available from environment
const githubParams = { ...params };
if (this.agent?.env?.GITHUB_TOKEN) {
  console.log("Using GitHub token from environment");
  githubParams.token = this.agent.env.GITHUB_TOKEN;
}
```

## Testing

This implementation has been fixed to correctly use the SSE transport with the MCP GitHub server, which should resolve the connection issues previously encountered. The agent should now be able to successfully fetch content from GitHub repositories.

Example usage:

```
// Fetch the README from the OpenAgentsInc/openagents repository
githubGetFile({
  owner: "openagentsinc",
  repo: "openagents",
  path: "README.md",
  branch: "main"
})
```
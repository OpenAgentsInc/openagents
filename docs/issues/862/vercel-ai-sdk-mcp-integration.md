# Vercel AI SDK MCP Integration

## Overview

This document describes the implementation of Vercel AI SDK's Model Context Protocol (MCP) client in the OpenAgents codebase, replacing our custom Cloudflare MCP handling. This change simplifies our codebase, improves maintainability, and ensures compatibility with the evolving MCP protocol.

## Implementation Details

### 1. Dependencies and Imports

The implementation uses the experimental MCP client from Vercel AI SDK:

```typescript
import {
  experimental_createMCPClient as createMCPClient,
  // Other imports...
} from "ai";
```

### 2. MCP Client Initialization

The MCP client is initialized in the `onStart` method of the `Coder` class:

```typescript
async onStart() {
  console.log("Initializing Coder agent...");

  // Initialize combinedTools with built-in tools
  this.combinedTools = { ...builtInTools };

  try {
    // Create MCP client with a simplified transport configuration
    this.mcpClient = await createMCPClient({
      transport: {
        type: 'sse' as const,
        url: "https://mcp-github.openagents.com/sse"
      },
      name: "coder"
    });
    
    console.log("Vercel AI SDK MCP client initialized");

    // Fetch tools from MCP
    await this.refreshServerData();
    
    // Log tool state for debugging
    console.log("MCP tools loaded, tool state:", {
      combinedToolCount: this.combinedTools ? Object.keys(this.combinedTools).length : 0,
      toolNames: this.combinedTools ? Object.keys(this.combinedTools) : []
    });
  } catch (error) {
    console.error("Failed to initialize MCP client:", error);
    // Continue with just the built-in tools if MCP client fails
  }
}
```

### 3. Tool Retrieval and Processing

The implementation uses the Vercel AI SDK's `tools()` method to automatically convert MCP tools, with additional wrappers for fallback handling:

```typescript
async refreshServerData() {
  // Skip if MCP client isn't initialized
  if (!this.mcpClient) {
    console.log("MCP client not initialized, skipping tool refresh");
    return;
  }

  try {
    console.log("Fetching MCP tools...");
    
    // Get tools from the MCP client using Vercel AI SDK
    // This automatically converts MCP tools to Vercel AI SDK tools
    const mcpTools = await this.mcpClient.tools();
    
    console.log("MCP tools retrieved:", Object.keys(mcpTools));
    
    // Filter to only use specific tools
    const filteredTools: Record<string, any> = {};
    for (const [name, toolDef] of Object.entries(mcpTools)) {
      if (name === 'get_file_contents') {
        console.log(`Including MCP tool: ${name}`);
        
        // Type assertion for toolDef
        const typedToolDef = toolDef as {
          description: string;
          parameters: any;
          execute: (args: any, options?: any) => Promise<any>;
        };
        
        // Add custom wrapper with fallback
        filteredTools[name] = tool({
          description: typedToolDef.description,
          parameters: typedToolDef.parameters,
          execute: async (args, options) => {
            try {
              // Try to use the original tool's execute function
              return await typedToolDef.execute(args, options);
            } catch (error: unknown) {
              // Implement fallback to direct GitHub API...
              // Fallback code omitted for brevity
            }
          }
        });
      }
    }

    // Combine built-in tools with MCP tools
    this.combinedTools = {
      ...builtInTools,
      ...filteredTools,
      weather: weatherTool // Always include the weather tool for testing
    };
  } catch (error) {
    // Handle errors and fall back to built-in tools
  }
}
```

### 4. GitHub Token Handling

The implementation manages GitHub tokens through a dedicated method that updates the MCP client:

```typescript
async updateGitHubToken(token: string) {
  this.githubToken = token;
  console.log("GitHub token updated");
  
  if (this.mcpClient) {
    try {
      // Create a new MCP client with the token in transport
      this.mcpClient = await createMCPClient({
        transport: {
          type: 'sse' as const,
          url: "https://mcp-github.openagents.com/sse"
        },
        name: "coder"
      });
      
      console.log("MCP client re-initialized with GitHub token");
      
      // Refresh tools with the new authenticated client
      await this.refreshServerData();
    } catch (error) {
      console.error("Failed to update MCP client with GitHub token:", error);
    }
  }
}
```

The token is received through WebSocket messages:

```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  // Call the parent method first
  await super.onMessage(connection, message);
  
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

### 5. Tool Usage in Chat

The implementation provides all tools to the AI model without filtering:

```typescript
async generateTools() {
  if (!this.combinedTools || Object.keys(this.combinedTools).length === 0) {
    console.log("TOOLS: No tools available, using weather example only");
    return {
      weather: tool({
        description: 'Get the weather in a location',
        parameters: z.object({
          location: z.string().describe('The location to get the weather for'),
        }),
        execute: async ({ location }) => ({
          location,
          temperature: 72 + Math.floor(Math.random() * 21) - 10,
        }),
      }),
    };
  }
  
  // Get just the tool names for cleaner logging
  const toolNames = Object.keys(this.combinedTools);
  console.log("TOOLS: Using combined tools:", toolNames);
  
  // In this implementation, we'll use all available tools
  // This works because the Vercel AI SDK tools are already properly formatted
  return this.combinedTools;
}
```

## Benefits of this Implementation

1. **Simplified Codebase**: The Vercel AI SDK handles most of the complexity
2. **Standardized Integration**: Uses well-maintained, documented interfaces
3. **Type Safety**: Better TypeScript integration throughout
4. **Easier Maintenance**: Less custom code to maintain
5. **Future Compatibility**: Easier to upgrade as the MCP protocol evolves
6. **Robust Error Handling**: Maintains fallback mechanisms for critical operations

## Known Issues and Limitations

1. **Experimental API**: The Vercel AI SDK MCP client is still marked as experimental
2. **GitHub Token Usage**: Currently, the GitHub token is not passed directly in the transport configuration due to type limitations
3. **TypeScript Errors**: There are some TypeScript errors related to imports and references to other files in the codebase

## Future Improvements

1. **Update to Stable API**: When Vercel AI SDK provides a stable MCP client API, migrate to it
2. **Better Token Handling**: Improve GitHub token handling when direct header configuration is supported
3. **Expanded Tool Support**: Add support for more MCP tools beyond `get_file_contents`
4. **Type Fixes**: Resolve remaining TypeScript errors throughout the codebase
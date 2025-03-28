# MCP Integration with Chat Server

This document details the implementation of Model Context Protocol (MCP) in the OpenAgents chat server, focusing on how GitHub tools are integrated with the LLM chat interface.

## Architecture Overview

The integration follows a layered architecture:

1. **Chat Client**: Sends messages and receives streaming responses
2. **Chat Server**: Processes LLM responses and routes tool calls
3. **MCP Client**: Connects to MCP servers and executes tools
4. **MCP Server**: Provides the actual tool implementations (GitHub, etc.)

```
┌───────────────┐     ┌──────────────────────────────────┐
│               │     │               CHAT SERVER        │
│  Chat Client  │────▶│                                  │
│               │     │  ┌────────────┐  ┌────────────┐  │
└───────┬───────┘     │  │            │  │            │  │
        │             │  │  LLM API   │  │ MCP Client │──┼───┐
        │             │  │            │  │            │  │   │
        │             │  └────────────┘  └────────────┘  │   │
        │             │                                  │   │
        │             └──────────────────────────────────┘   │
        │                                                    │
        │                                                    │
        │             ┌──────────────────────────────────┐   │
        │             │                                  │   │
        └────────────▶│       GitHub MCP Server         │◀──┘
                      │                                  │
                      └──────────────────────────────────┘
```

## Implementation Details

### 1. MCP Client Manager

The `McpClientManager` class is implemented to:
- Connect to multiple MCP servers
- Discover and register available tools
- Route tool calls to the appropriate server
- Handle authentication pass-through

```typescript
export class McpClientManager {
  // Maps of connected clients and registered tools
  private clients: Map<string, Client> = new Map();
  private toolRegistry: Map<string, { server: string; description: string }> = new Map();
  
  // Connect to a server and discover its tools
  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    // Implementation details...
  }
  
  // Call a tool with arguments and optional auth token
  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
    // Implementation details...
  }
  
  // Other methods...
}
```

### 2. Tool Handling Flow

1. **Tool Definition**: The chat server exposes GitHub tool definitions to the LLM
2. **Tool Call**: When the LLM generates a tool call, it's intercepted in the stream
3. **Routing**: The tool call is routed to the appropriate MCP server
4. **Execution**: The MCP client executes the tool with the provided auth token
5. **Response**: The tool result is injected back into the stream

```typescript
// Stream interceptor for tool calls
const interceptStream = async function*(stream: ReadableStream) {
  const reader = stream.getReader();
  
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      
      // Check if chunk contains a complete tool call
      if (value?.toolCalls?.length > 0 && !value.isPartial) {
        const toolCall = {
          toolCallId: value.toolCalls[0].toolCallId,
          toolName: value.toolCalls[0].toolName,
          args: value.toolCalls[0].args
        };
        
        // Process the tool call with auth token
        const toolResult = await processToolCall(toolCall, authToken);
        
        // Create a modified chunk with tool result
        yield {
          ...value,
          toolResults: [toolResult]
        };
      } else {
        // Pass through the chunk unchanged
        yield value;
      }
    }
  } finally {
    reader.releaseLock();
  }
};
```

### 3. Authentication Flow

Authentication is handled via a pass-through model:

1. The client includes a GitHub token in the Authorization header
2. The chat server extracts this token but does not store it
3. The token is passed along with each tool call to the GitHub MCP server
4. The GitHub MCP server uses the token to authenticate API requests

```typescript
// In the chat server endpoint
app.post('/', async c => {
  // Extract auth token from Authorization header
  const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
  
  // Later passed to tool calls
  const toolResult = await processToolCall(toolCall, authToken);
});

// In the MCP client
async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
  // Add token to args if provided
  const toolArgs = token ? { ...args, token } : args;
  
  // Call tool with the auth token included
  const result = await client.callTool({
    name: toolName,
    arguments: toolArgs,
  });
}
```

## Streaming Response Format

The chat server uses the Vercel AI SDK data streaming format for compatibility with clients:

```javascript
// Example chunk with a tool call
{
  "id": "chatcmpl-123",
  "object": "chat.completion.chunk",
  "choices": [{
    "index": 0,
    "delta": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "index": 0,
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "get_issue",
          "arguments": "{\"owner\":\"OpenAgentsInc\",\"repo\":\"openagents\",\"issue_number\":789}"
        }
      }]
    }
  }]
}

// Example chunk with a tool result
{
  "id": "chatcmpl-123",
  "object": "chat.completion.chunk",
  "choices": [{
    "index": 0,
    "delta": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "index": 0,
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "get_issue",
          "arguments": "{\"owner\":\"OpenAgentsInc\",\"repo\":\"openagents\",\"issue_number\":789}"
        }
      }],
      "tool_results": [{
        "tool_call_id": "call_abc123",
        "role": "tool",
        "name": "get_issue", 
        "content": "{\"html_url\":\"https://github.com/OpenAgentsInc/openagents/issues/789\",\"number\":789,\"title\":\"Integration Plan: GitHub MCP Tools into Chat Server\"}"
      }]
    }
  }]
}
```

## Security Considerations

1. **Token Handling**:
   - Tokens are never stored on the server
   - Passed through to the MCP server only when needed
   - Not logged or persisted

2. **Request Validation**:
   - All tool calls are validated
   - Tool schemas are enforced

3. **Error Handling**:
   - Tool errors are caught and formatted
   - Errors don't expose sensitive information

## Limitations and Future Work

1. **Dynamic Tool Discovery**:
   - Currently using predefined tool schemas
   - Future: convert MCP tool schemas to LLM tool format dynamically

2. **Multi-Server Support**:
   - Currently only GitHub MCP server
   - Design supports multiple MCP servers

3. **Authentication Flows**:
   - Simple token pass-through
   - Future: OAuth flows, refresh tokens

4. **Testing and Monitoring**:
   - Add comprehensive test suite
   - Add monitoring for tool usage and errors

## References

- [Model Context Protocol Documentation](docs/mcp.md)
- [Vercel AI SDK Streaming Protocol](https://sdk.vercel.ai/docs/api-reference/streaming-format)
- [GitHub API Documentation](https://docs.github.com/en/rest)
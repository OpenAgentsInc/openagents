# GitHub MCP Tools Integration into Chat Server

## Architecture Design

### Overview

This document details the technical design for integrating GitHub MCP tools into the OpenAgents chat server. The integration enables LLMs to interact with GitHub repositories through a centralized MCP client implementation in the chat server.

```
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│               │     │               │     │               │
│  Chat Client  │────▶│  Chat Server  │────▶│  LLM Service  │
│               │     │               │     │               │
└───────┬───────┘     └───────┬───────┘     └───────────────┘
        │                     │
        │                     │    ┌───────────────┐
        │                     │    │               │
        │                     └───▶│  MCP Client   │
        │                          │  (Centralized)│
        │                          └───────┬───────┘
        │                                  │
┌───────▼───────┐                  ┌───────▼───────┐
│               │                  │               │
│  Auth Token   │─────────────────▶│  GitHub MCP   │
│  Storage      │                  │  Server       │
│               │                  │               │
└───────────────┘                  └───────────────┘
```

### Components

1. **Chat Server**
   - Hosts centralized MCP client
   - Routes tool calls to appropriate MCP servers
   - Manages authentication tokens
   - Streams responses back to clients

2. **MCP Client**
   - Connects to multiple MCP servers
   - Discovers and caches available tools
   - Handles tool calling and streaming responses
   - Provides a unified interface for LLM interactions

3. **Authentication**
   - Securely passes tokens from clients to MCP servers
   - No persistent storage of tokens on server
   - Pass-through authentication model

4. **Streaming**
   - Real-time streaming of tool execution results
   - Integration with Vercel AI SDK for consistent streaming

## Implementation Details

### 1. MCP Client Implementation

```typescript
// apps/chatserver/src/mcp/client.ts

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@openagents/core";

export class McpClientManager {
  private clients: Map<string, Client> = new Map();
  private toolRegistry: Map<string, { server: string, description: string }> = new Map();

  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    if (this.clients.has(serverName)) {
      return this.clients.get(serverName)!;
    }

    const transport = new SSEClientTransport(new URL(serverUrl));
    const client = new Client(
      { name: 'chatserver', version: '0.0.1' },
      {
        capabilities: {
          sampling: {},
          roots: { listChanged: true }
        }
      }
    );

    await client.connect(transport);
    this.clients.set(serverName, client);
    
    // Discover tools
    await this.discoverTools(client, serverName);
    
    return client;
  }

  async discoverTools(client: Client, serverName: string): Promise<void> {
    try {
      const tools = await client.listTools();
      
      tools.forEach(tool => {
        this.toolRegistry.set(tool.name, {
          server: serverName,
          description: tool.description || ''
        });
      });
      
      console.log(`Discovered ${tools.length} tools from ${serverName}`);
    } catch (error) {
      console.error(`Failed to discover tools from ${serverName}:`, error);
    }
  }

  getToolServer(toolName: string): string | undefined {
    return this.toolRegistry.get(toolName)?.server;
  }

  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
    const serverName = this.getToolServer(toolName);
    if (!serverName) {
      throw new Error(`Tool ${toolName} not found in any connected MCP server`);
    }

    const client = this.clients.get(serverName);
    if (!client) {
      throw new Error(`MCP server ${serverName} not connected`);
    }

    // Add token to args if provided
    const toolArgs = token ? { ...args, token } : args;
    
    // Call tool with streaming support
    return client.callTool({
      name: toolName,
      arguments: toolArgs
    });
  }
}
```

### 2. Enhanced Chat Server

```typescript
// apps/chatserver/src/index.ts

import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { McpClientManager } from './mcp/client';

interface Env {
  AI: typeof AI;
}

interface ChatMessage {
  content: string;
  role: 'user' | 'assistant';
}

interface ToolCall {
  toolName: string;
  args: Record<string, any>;
}

const app = new Hono<{ Bindings: Env }>();
const mcpManager = new McpClientManager();

// Initialize MCP connections
async function initMcp() {
  try {
    await mcpManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('Connected to GitHub MCP server');
  } catch (error) {
    console.error('Failed to connect to GitHub MCP server:', error);
  }
}

// Call in a non-blocking way
void initMcp();

// Enable CORS
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

app.get('/', c => c.text('200'));

app.post('/', async c => {
  const body = await c.req.json();
  const messages = body.messages || [];
  const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
  
  const workersai = createWorkersAI({ binding: c.env.AI });
  const result = streamText({
    model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
    messages,
    tools: extractToolsDefinition(mcpManager),
    toolCallStreaming: true
  });

  // Set up an interceptor for tool calls
  const interceptStream = async function*(stream: ReadableStream) {
    const reader = stream.getReader();
    
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        
        // Process potential tool calls
        const processedValue = await processToolCall(value, authToken);
        yield processedValue;
      }
    } finally {
      reader.releaseLock();
    }
  };

  // Mark the response as a v1 data stream
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/plain; charset=utf-8');

  return stream(c, stream => stream.pipe(
    interceptStream(result.toDataStream())
  ));
});

// Process tool calls in the stream
async function processToolCall(chunk: any, authToken?: string) {
  // If chunk contains a tool call
  if (chunk?.toolCalls && chunk.toolCalls.length > 0) {
    const toolCall = chunk.toolCalls[0];
    
    try {
      // Call the tool via MCP
      const result = await mcpManager.callTool(
        toolCall.toolName,
        toolCall.args,
        authToken
      );
      
      // Add the tool result to the chunk
      return {
        ...chunk,
        toolResults: [{
          toolCallId: toolCall.toolCallId,
          toolName: toolCall.toolName,
          result: result
        }]
      };
    } catch (error) {
      console.error(`Tool call error for ${toolCall.toolName}:`, error);
      return {
        ...chunk,
        toolResults: [{
          toolCallId: toolCall.toolCallId,
          toolName: toolCall.toolName,
          result: { error: 'Tool execution failed' }
        }]
      };
    }
  }
  
  return chunk;
}

// Extract tool definitions from MCP manager for LLM
function extractToolsDefinition(manager: McpClientManager) {
  // In reality, we would get these from the MCP toolRegistry
  // This is a simplified version
  return [
    {
      name: "create_issue",
      description: "Create a new issue in a GitHub repository",
      parameters: {
        type: "object",
        properties: {
          owner: { type: "string", description: "Repository owner" },
          repo: { type: "string", description: "Repository name" },
          title: { type: "string", description: "Issue title" },
          body: { type: "string", description: "Issue body" }
        },
        required: ["owner", "repo", "title"]
      }
    },
    // Other tool definitions would be added here
  ];
}

export default app;
```

### 3. Authentication Flow

```typescript
// This code would be part of the chat server implementation

// Client-side header setup
const headers = {
  'Content-Type': 'application/json',
  'Authorization': `Bearer ${githubToken}`
};

// Server-side token extraction
app.post('/api/chat', async c => {
  // Extract token from Authorization header
  const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
  
  // Pass token to MCP client for tool calls
  const result = await mcpManager.callTool('create_issue', {
    owner: 'OpenAgentsInc',
    repo: 'openagents',
    title: 'Test issue'
  }, authToken);
  
  // Rest of handler
  // ...
});
```

## Streaming Response Format

The chat server will use the Vercel AI SDK streaming format for consistency:

```json
{
  "id": "msg_1a2b3c4d",
  "role": "assistant",
  "content": "I'll help you create that issue.",
  "toolCalls": [
    {
      "toolCallId": "call_1234567",
      "toolName": "create_issue",
      "args": {
        "owner": "OpenAgentsInc",
        "repo": "openagents",
        "title": "New bug report",
        "body": "Encountered an error when..."
      }
    }
  ],
  "toolResults": [
    {
      "toolCallId": "call_1234567",
      "toolName": "create_issue",
      "result": {
        "html_url": "https://github.com/OpenAgentsInc/openagents/issues/42",
        "number": 42,
        "title": "New bug report"
      }
    }
  ]
}
```

## Next Steps

1. Implement the MCP client manager in the chat server
2. Enhance the current chat endpoint to support tool calling
3. Add authentication pass-through for GitHub tokens
4. Test with various GitHub operations via the MCP server
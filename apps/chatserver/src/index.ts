import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
import { extractToolDefinitions, processToolCall } from './mcp/tools';

interface Env {
  AI: typeof AI;
}

interface ChatMessage {
  content: string;
  role: 'user' | 'assistant';
}

interface ToolCallPayload {
  toolCallId: string;
  toolName: string;
  args: Record<string, any>;
}

const app = new Hono<{ Bindings: Env }>();

// Initialize MCP connections
async function initMcp() {
  try {
    await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('Connected to GitHub MCP server');
  } catch (error) {
    console.error('Failed to connect to GitHub MCP server:', error);
  }
}

// Call in a non-blocking way to avoid delaying server startup
void initMcp();

// Enable CORS for all routes
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check endpoint
app.get('/', c => c.text('200 OK'));

// Main chat endpoint
app.post('/', async c => {
  const body = await c.req.json();
  const messages = body.messages || [];
  
  // Extract auth token from Authorization header
  const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
  
  // Get AI model from Cloudflare Workers AI
  const workersai = createWorkersAI({ binding: c.env.AI });
  
  // Get tool definitions for the LLM
  const tools = extractToolDefinitions();
  
  // Create streaming response with tools enabled
  const result = streamText({
    model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
    messages,
    tools,
    toolCallStreaming: true
  });

  // Set up an interceptor for tool calls
  const interceptStream = async function*(stream: ReadableStream) {
    const reader = stream.getReader();
    let toolCallBuffer: ToolCallPayload | null = null;
    
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        
        // Check if chunk contains a complete tool call
        if (value?.toolCalls?.length > 0 && !value.isPartial) {
          toolCallBuffer = {
            toolCallId: value.toolCalls[0].toolCallId,
            toolName: value.toolCalls[0].toolName,
            args: value.toolCalls[0].args
          };
          
          // Process the tool call with auth token
          const toolResult = await processToolCall(toolCallBuffer, authToken);
          
          // Create a modified chunk with tool result
          const resultChunk = {
            ...value,
            toolResults: [toolResult]
          };
          
          yield resultChunk;
          toolCallBuffer = null;
        } else {
          // Pass through the chunk unchanged
          yield value;
        }
      }
    } finally {
      reader.releaseLock();
    }
  };

  // Mark the response as a v1 data stream
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/plain; charset=utf-8');

  return stream(c, async stream => {
    const transformedStream = interceptStream(result.toDataStream());
    for await (const chunk of transformedStream) {
      await stream.write(chunk);
    }
  });
});

export default app;
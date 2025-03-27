import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
import { extractToolDefinitions, processToolCall, type ToolCallPayload, type ToolDefinition } from './mcp/tools';

interface Env {
  AI: typeof AI;
}

interface ChatMessage {
  content: string;
  role: 'user' | 'assistant';
}

type AIStreamChunk = Record<string, any>;

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
  console.log("🚀 Chat request received");
  
  try {
    const body = await c.req.json();
    console.log("📝 Request body:", JSON.stringify(body).substring(0, 200));
    
    const messages = body.messages || [];
    console.log(`📨 Message count: ${messages.length}`);
    
    // Extract auth token from Authorization header
    const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
    console.log(`🔑 Auth token present: ${!!authToken}`);
    
    // Get AI model from Cloudflare Workers AI
    const workersai = createWorkersAI({ binding: c.env.AI });
    
    // Get tool definitions for the LLM
    const tools = extractToolDefinitions();
    console.log(`🛠️ Available tools: ${tools.length}`);
    
    console.log("🔄 Initializing MCP connection");
    // Force MCP connection to ensure it's ready
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      const allTools = mcpClientManager.getAllTools();
      console.log(`✅ MCP Connected, discovered ${allTools.length} tools`);
    } catch (mcpError) {
      console.error("❌ MCP Connection failed:", mcpError);
    }
    
    // Create streaming response with tools enabled
    console.log("🎬 Starting streaming response");
    const result = streamText({
      model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
      messages,
      tools: tools as any, // Type casting to avoid ToolSet issues
      toolCallStreaming: true
    });

  // Set up an interceptor for tool calls
  const interceptStream = async function*(stream: ReadableStream<any>) {
    const reader = stream.getReader();
    let toolCallBuffer: ToolCallPayload | null = null;
    
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          console.log("🏁 Stream completed");
          break;
        }
        
        // Log chunk summary (without entire content)
        console.log(`📦 Received chunk: ${JSON.stringify({
          ...value,
          content: value?.content ? "(content present)" : undefined,
          toolCalls: value?.toolCalls?.length ? `(${value.toolCalls.length} calls)` : undefined,
          isPartial: value?.isPartial
        })}`);
        
        // Check if chunk contains a complete tool call
        if (value?.toolCalls?.length > 0 && !value.isPartial) {
          console.log(`🔨 Tool call detected: ${value.toolCalls[0].toolName}`);
          
          toolCallBuffer = {
            toolCallId: value.toolCalls[0].toolCallId,
            toolName: value.toolCalls[0].toolName,
            args: value.toolCalls[0].args
          };
          
          console.log(`⚙️ Processing tool call: ${toolCallBuffer.toolName} with args: ${JSON.stringify(toolCallBuffer.args).substring(0, 100)}`);
          
          // Process the tool call with auth token
          const toolResult = await processToolCall(toolCallBuffer, authToken);
          console.log(`🎯 Tool result received: ${JSON.stringify(toolResult).substring(0, 100)}...`);
          
          // Create a modified chunk with tool result
          const resultChunk = {
            ...value,
            toolResults: toolResult ? [toolResult] : []
          };
          
          yield resultChunk;
          toolCallBuffer = null;
        } else {
          // Pass through the chunk unchanged
          yield value as AIStreamChunk;
        }
      }
    } catch (error) {
      console.error("💥 Error in stream processing:", error);
      throw error;
    } finally {
      console.log("🔄 Releasing stream reader");
      reader.releaseLock();
    }
  };

  // Mark the response as a v1 data stream
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/plain; charset=utf-8');

  return stream(c, async stream => {
    try {
      console.log("🔄 Starting stream processing");
      const transformedStream = interceptStream(result.toDataStream());
      
      console.log("🌊 Beginning stream iteration");
      for await (const chunk of transformedStream) {
        console.log("📤 Writing chunk to response");
        await stream.write(chunk);
      }
      console.log("✅ Stream processing complete");
    } catch (error) {
      console.error("💥 Critical error in stream handling:", error);
      // Return error to client
      await stream.write({ error: "Stream processing failed" });
    }
  });
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;
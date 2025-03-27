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
        
        // Log chunk summary with detailed content
        console.log(`📦 Received chunk: ${JSON.stringify(value)}`);
        
        // Decode the array if it's a buffer-like object
        let processedValue = value;
        if (value && typeof value === 'object' && '0' in value) {
          try {
            const chars = Object.values(value).map(v => Number(v));
            const decodedText = String.fromCharCode(...chars);
            console.log(`🔍 Decoded chunk text: ${decodedText}`);
            
            // Try to parse the decoded text
            try {
              const parsed = JSON.parse(decodedText);
              processedValue = parsed;
              console.log(`🔄 Parsed chunk: ${JSON.stringify(parsed)}`);
            } catch (parseError) {
              console.log(`⚠️ Not valid JSON: ${decodedText}`);
            }
          } catch (decodeError) {
            console.error(`❌ Failed to decode chunk: ${decodeError}`);
          }
        }
        
        // Check if chunk contains a complete tool call
        if (processedValue?.toolCalls?.length > 0 && !processedValue.isPartial) {
          console.log(`🔨 Tool call detected: ${processedValue.toolCalls[0].toolName}`);
          
          toolCallBuffer = {
            toolCallId: processedValue.toolCalls[0].toolCallId,
            toolName: processedValue.toolCalls[0].toolName,
            args: processedValue.toolCalls[0].args
          };
          
          console.log(`⚙️ Processing tool call: ${toolCallBuffer.toolName} with args: ${JSON.stringify(toolCallBuffer.args).substring(0, 100)}`);
          
          // Process the tool call with auth token
          const toolResult = await processToolCall(toolCallBuffer, authToken);
          console.log(`🎯 Tool result received: ${JSON.stringify(toolResult).substring(0, 100)}...`);
          
          // Create a modified chunk with tool result
          const resultChunk = {
            ...processedValue,
            toolResults: toolResult ? [toolResult] : []
          };
          
          yield resultChunk;
          toolCallBuffer = null;
        } else {
          // Pass through the chunk unchanged
          yield processedValue as AIStreamChunk;
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

  // Mark the response as a SSE stream
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/event-stream');
  c.header('Cache-Control', 'no-cache');
  c.header('Connection', 'keep-alive');

  return stream(c, async stream => {
    try {
      console.log("🔄 Starting stream processing");
      const transformedStream = interceptStream(result.toDataStream());
      
      console.log("🌊 Beginning stream iteration");
      for await (const chunk of transformedStream) {
        console.log("📤 Writing chunk to response");
        // Format properly for SSE (Server-Sent Events) for Vercel AI SDK
        await stream.write(`data: ${JSON.stringify(chunk)}\n\n`);
      }
      console.log("✅ Stream processing complete");
    } catch (error) {
      console.error("💥 Critical error in stream handling:", error);
      // Return error to client in a format that matches other responses
      await stream.write(`data: ${JSON.stringify({ error: "Stream processing failed" })}\n\n`);
    }
  });
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;